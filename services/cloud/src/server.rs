//! Starting and stopping the server.
//!
//! Shutdown is graceful: readiness turns to 503 so the platform stops
//! routing new connections, every WebSocket is closed with 1012 (service
//! restart) so clients reconnect to the next instance, every room is
//! persisted and compacted, and queued webhooks get a few seconds to go out.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

use crate::api;
use crate::app::App;
use crate::app::SharedApp;
use crate::config::Config;
use crate::metering::Meter;
use crate::metering::stripe::StripeMeter;
use crate::metering::stripe::StripeWorker;
use crate::store::Store;
use crate::store::StoreError;
use crate::sync::Rooms;
use crate::sync::WebhookWorker;
use crate::sync::Webhooks;

/// How long shutdown waits for sockets to close.
const SOCKET_DRAIN: Duration = Duration::from_secs(10);
/// How long shutdown waits for queued webhooks.
const WEBHOOK_DRAIN: Duration = Duration::from_secs(5);

/// Background workers that finish their queues at shutdown.
struct Workers {
	webhooks: WebhookWorker,
	stripe: Option<StripeWorker>,
}

/// A failure to start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
	/// The listener could not bind.
	#[error("could not listen: {0}")]
	Bind(#[from] std::io::Error),
	/// The store could not be reached or migrated.
	#[error(transparent)]
	Store(#[from] StoreError),
}

/// What happened during shutdown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShutdownReport {
	/// Rooms whose changes could not be persisted.
	pub persist_failures: usize,
}

/// A running server.
#[derive(Debug)]
pub struct Server {
	addr: SocketAddr,
	app: SharedApp,
	stop: watch::Sender<bool>,
	task: JoinHandle<ShutdownReport>,
}

impl Server {
	/// Start with the store `config` names: Postgres at `DATABASE_URL`, or
	/// memory when it is unset.
	pub async fn start(config: Config) -> Result<Self, StartError> {
		let store = if let Some(url) = &config.database_url {
			Store::postgres(url.expose()).await?
		} else {
			tracing::warn!(
				"DATABASE_URL is unset; documents live in memory and are lost on restart"
			);
			Store::memory()
		};

		Self::start_with_store(config, store).await
	}

	/// Start over `store`.
	pub async fn start_with_store(config: Config, store: Store) -> Result<Self, StartError> {
		let listener = TcpListener::bind(config.bind).await?;
		let addr = listener.local_addr()?;
		let (stop, stopped) = watch::channel(false);

		let (webhooks, worker) = Webhooks::start(store.clone());
		let (stripe, stripe_worker) = config.stripe.clone().map(StripeMeter::start).unzip();
		let meter = Meter::new(store.clone(), stripe);
		let rooms = Rooms::new(store.clone(), config.sync.clone(), webhooks);
		let sockets = TaskTracker::new();
		let app = Arc::new(App::new(
			config,
			store,
			(rooms, meter),
			stopped.clone(),
			sockets.clone(),
		));
		let workers = Workers {
			webhooks: worker,
			stripe: stripe_worker,
		};
		let task = tokio::spawn(run(app.clone(), listener, stopped, sockets, workers));
		tracing::info!(%addr, role = ?app.config.role, "canvist cloud listening");
		Ok(Self {
			addr,
			app,
			stop,
			task,
		})
	}

	/// Where the server listens.
	pub fn addr(&self) -> SocketAddr {
		self.addr
	}

	/// The shared state, for tests and embedding.
	pub fn app(&self) -> &SharedApp {
		&self.app
	}

	/// Shut down gracefully.
	pub async fn shutdown(self) -> ShutdownReport {
		let _ = self.stop.send(true);
		drop(self.app);
		self.task.await.unwrap_or_default()
	}

	/// Run until SIGTERM or Ctrl-C, then shut down gracefully.
	pub async fn run_until_signal(self) -> ShutdownReport {
		wait_for_signal().await;
		tracing::info!("shutting down");
		self.shutdown().await
	}
}

async fn run(
	app: SharedApp,
	listener: TcpListener,
	stopped: watch::Receiver<bool>,
	sockets: TaskTracker,
	workers: Workers,
) -> ShutdownReport {
	let flush = tokio::spawn(flush_loop(app.clone(), stopped.clone()));
	let router = api::router(app.clone());
	let mut until_stopped = stopped.clone();
	let served = axum::serve(listener, router)
		.with_graceful_shutdown(async move {
			let _ = until_stopped.wait_for(|stopping| *stopping).await;
		})
		.await;

	if let Err(error) = served {
		tracing::error!(%error, "the HTTP server failed");
	}

	let _ = flush.await;
	sockets.close();

	if tokio::time::timeout(SOCKET_DRAIN, sockets.wait())
		.await
		.is_err()
	{
		tracing::warn!("sockets still open after the drain period");
	}

	let persist_failures = app.rooms.flush_all().await;
	drop(app);
	workers.webhooks.drain(WEBHOOK_DRAIN).await;

	if let Some(stripe) = workers.stripe {
		stripe.drain(WEBHOOK_DRAIN).await;
	}

	tracing::info!(persist_failures, "shutdown complete");
	ShutdownReport { persist_failures }
}

async fn flush_loop(app: SharedApp, mut stopped: watch::Receiver<bool>) {
	let mut interval = tokio::time::interval(app.config.sync.flush_interval);
	interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

	loop {
		tokio::select! {
			_ = interval.tick() => app.rooms.tick(Instant::now()).await,
			_ = stopped.changed() => break,
		}
	}
}

async fn wait_for_signal() {
	let interrupt = async {
		let _ = tokio::signal::ctrl_c().await;
	};

	#[cfg(unix)]
	let terminate = async {
		match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
			Ok(mut signal) => {
				signal.recv().await;
			}
			Err(error) => {
				tracing::error!(%error, "cannot listen for SIGTERM");
				std::future::pending::<()>().await;
			}
		}
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	tokio::select! {
		() = interrupt => {}
		() = terminate => {}
	}
}
