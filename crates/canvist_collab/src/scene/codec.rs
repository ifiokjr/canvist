//! JSON values as Yjs values, and the node position register.

use std::collections::HashMap;
use std::sync::Arc;

use canvist_scene::FracKey;
use canvist_scene::NodeId;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value as Json;
use yrs::Any;
use yrs::any::Number as AnyNumber;

/// A JSON value as a Yjs value. Integers stay integers, so JavaScript peers
/// read the same numbers.
pub(crate) fn json_to_any(value: &Json) -> Any {
	match value {
		Json::Null => Any::Null,
		Json::Bool(value) => Any::Bool(*value),
		Json::Number(number) => {
			number.as_i64().map_or_else(
				|| Any::Number(AnyNumber::Float(number.as_f64().unwrap_or(0.0))),
				|int| Any::Number(AnyNumber::Int(int)),
			)
		}
		Json::String(text) => Any::String(text.as_str().into()),
		Json::Array(items) => Any::Array(items.iter().map(json_to_any).collect()),
		Json::Object(map) => {
			let entries: HashMap<String, Any> = map
				.iter()
				.map(|(key, value)| (key.clone(), json_to_any(value)))
				.collect();
			Any::Map(Arc::new(entries))
		}
	}
}

/// A Yjs value as JSON. `undefined` becomes `null`; binary buffers, which
/// scene properties never hold, become `null` too.
pub(crate) fn any_to_json(any: &Any) -> Json {
	match any {
		Any::Null | Any::Undefined | Any::Buffer(_) => Json::Null,
		Any::Bool(value) => Json::Bool(*value),
		Any::Number(AnyNumber::Int(int)) => Json::Number((*int).into()),
		Any::Number(AnyNumber::Float(float)) => {
			Number::from_f64(*float).map_or(Json::Null, Json::Number)
		}
		Any::String(text) => Json::String(text.to_string()),
		Any::Array(items) => Json::Array(items.iter().map(any_to_json).collect()),
		Any::Map(entries) => {
			let map: Map<String, Json> = entries
				.iter()
				.map(|(key, value)| (key.clone(), any_to_json(value)))
				.collect();
			Json::Object(map)
		}
	}
}

/// A node's position register: where it is, plus what breaks move cycles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pos {
	/// The parent.
	pub(crate) parent: NodeId,
	/// The key among the parent's children.
	pub(crate) key: FracKey,
	/// A Lamport clock: one more than the largest clock the writer had seen.
	pub(crate) clock: u64,
	/// The writer's scene client id, which breaks clock ties.
	pub(crate) client: u32,
	/// The parent before this write.
	pub(crate) previous: NodeId,
}

impl Pos {
	/// The order in which concurrent moves lose: the greatest loses.
	pub(crate) fn rank(&self) -> (u64, u32) {
		(self.clock, self.client)
	}

	/// `[parent, key, clock, client, previous]`.
	pub(crate) fn to_any(&self) -> Any {
		Any::Array(Arc::from([
			Any::String(self.parent.to_string().into()),
			Any::String(self.key.as_str().into()),
			Any::Number(AnyNumber::Int(
				i64::try_from(self.clock).unwrap_or(i64::MAX),
			)),
			Any::Number(AnyNumber::Int(i64::from(self.client))),
			Any::String(self.previous.to_string().into()),
		]))
	}

	/// Read a register, or `None` when it is malformed.
	pub(crate) fn from_any(any: &Any) -> Option<Self> {
		let Any::Array(items) = any else {
			return None;
		};
		let [parent, key, clock, client, previous] = &**items else {
			return None;
		};

		Some(Self {
			parent: string(parent)?.parse().ok()?,
			key: FracKey::parse(string(key)?).ok()?,
			clock: u64::try_from(integer(clock)?).ok()?,
			client: u32::try_from(integer(client)?).ok()?,
			previous: string(previous)?.parse().ok()?,
		})
	}
}

fn string(any: &Any) -> Option<&str> {
	match any {
		Any::String(text) => Some(text),
		_ => None,
	}
}

fn integer(any: &Any) -> Option<i64> {
	match any {
		Any::Number(AnyNumber::Int(int)) => Some(*int),
		Any::Number(AnyNumber::Float(float)) if float.fract() == 0.0 => Some(*float as i64),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	#[test]
	fn json_round_trips_through_any() {
		let value = json!({ "a": [1, 2.5, "x", null, true], "b": { "c": -3 } });

		assert_eq!(any_to_json(&json_to_any(&value)), value);
	}

	#[test]
	fn positions_round_trip() {
		let pos = Pos {
			parent: NodeId::new(1, 2),
			key: FracKey::first(),
			clock: 9,
			client: 4,
			previous: NodeId::ROOT,
		};

		assert_eq!(Pos::from_any(&pos.to_any()), Some(pos));
		assert_eq!(Pos::from_any(&Any::Null), None);
	}
}
