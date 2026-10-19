//! Encoding y-protocols messages, shared by both transports.
//!
//! A y-protocols message is what y-websocket sends as one binary frame;
//! Hocuspocus sends the same bytes after a document name.

use yrs::StateVector;
use yrs::sync::AwarenessUpdate;
use yrs::sync::Message;
use yrs::sync::SyncMessage;
use yrs::updates::encoder::Encode;

/// Sync step 1: the sender's state vector.
pub fn step1(state: &StateVector) -> Vec<u8> {
	Message::Sync(SyncMessage::SyncStep1(state.clone())).encode_v1()
}

/// Sync step 2: everything the receiver is missing.
pub fn step2(update: Vec<u8>) -> Vec<u8> {
	Message::Sync(SyncMessage::SyncStep2(update)).encode_v1()
}

/// An incremental document update.
pub fn update(update: Vec<u8>) -> Vec<u8> {
	Message::Sync(SyncMessage::Update(update)).encode_v1()
}

/// An awareness update.
pub fn awareness(update: AwarenessUpdate) -> Vec<u8> {
	Message::Awareness(update).encode_v1()
}
