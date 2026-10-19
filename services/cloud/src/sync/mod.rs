//! canvist Sync: a multi-tenant y-sync relay.
//!
//! Peers connect over y-websocket or Hocuspocus framing to a [`Room`] per
//! document. Rooms live in memory while anyone is connected, persist to the
//! store in the background, and are compacted and evicted once idle.

mod awareness;
mod room;
mod rooms;
mod session;
mod socket;

pub mod hocuspocus;
pub mod protocol;
pub mod validate;
pub mod webhooks;
pub mod ywebsocket;

pub use room::Changes;
pub use room::Fanout;
pub use room::Limits;
pub use room::Membership;
pub use room::PeerId;
pub use room::Room;
pub use rooms::RoomStats;
pub use rooms::Rooms;
pub use session::Reply;
pub use session::Session;
pub use webhooks::WebhookWorker;
pub use webhooks::Webhooks;
