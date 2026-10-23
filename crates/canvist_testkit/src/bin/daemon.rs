//! The testkit daemon binary. Never started by hand: harnesses start it
//! (see `daemon.rs`), and it exits on its own after an idle stretch.
//!
//! ```text
//! canvist_testkit_daemon daemon --target <loopback|ios|android> --root <workspace> --idle-secs <n>
//! ```

use std::path::PathBuf;
use std::time::Duration;

use canvist_testkit::daemon;

#[tokio::main(flavor = "current_thread")]
async fn main() {
	let mut target = None;
	let mut root = PathBuf::from(".");
	let mut idle = Duration::from_mins(5);

	let mut arguments = std::env::args().skip(1);
	while let Some(argument) = arguments.next() {
		match argument.as_str() {
			"daemon" => {}
			"--target" => {
				target = arguments.next().and_then(|name| name.parse().ok());
			}
			"--root" => {
				root = arguments
					.next()
					.map_or_else(|| PathBuf::from("."), PathBuf::from);
			}
			"--idle-secs" => {
				idle = arguments
					.next()
					.and_then(|value| value.parse().ok())
					.map_or(idle, Duration::from_secs);
			}
			other => {
				eprintln!("unknown argument: {other}");

				std::process::exit(2);
			}
		}
	}

	let Some(target) = target else {
		eprintln!("--target is required (loopback, ios, or android)");

		std::process::exit(2);
	};

	if let Err(error) = daemon::run(target, root, idle).await {
		eprintln!("the daemon could not run: {error}");

		std::process::exit(1);
	}
}
