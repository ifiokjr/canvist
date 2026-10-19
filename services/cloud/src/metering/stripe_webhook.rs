//! Stripe webhooks: verifying them, and finding what an event asks for.
//!
//! Stripe does not deliver events in order, so the server never applies an
//! event's snapshot of a subscription. It retrieves the subscription the
//! event names and applies its current state ([`super::billing`]), which
//! makes duplicates, retries, and reordering harmless. A completed annual
//! checkout names no subscription yet: the server creates one, idempotently.

use hmac::Hmac;
use hmac::Mac;
use serde_json::Value as Json;
use sha2::Sha256;

/// How old a signed event may be, in seconds, as Stripe's libraries allow.
pub const TOLERANCE_SECONDS: u64 = 300;

/// Why a webhook was refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum WebhookError {
	/// The `Stripe-Signature` header is missing, malformed, stale, or wrong.
	#[error("invalid Stripe signature")]
	Signature,
	/// The body is not a Stripe event.
	#[error("malformed Stripe event")]
	Payload,
}

/// Check `header` (`t=…,v1=…`) against `body` signed with `secret` at
/// `now` (Unix seconds).
pub fn verify(secret: &str, header: &str, body: &[u8], now: u64) -> Result<(), WebhookError> {
	let mut timestamp = None;
	let mut signatures = Vec::new();

	for part in header.split(',') {
		match part.trim().split_once('=') {
			Some(("t", value)) => timestamp = value.parse::<u64>().ok(),
			Some(("v1", value)) => signatures.push(value),
			_ => {}
		}
	}

	let timestamp = timestamp.ok_or(WebhookError::Signature)?;

	if now.abs_diff(timestamp) > TOLERANCE_SECONDS {
		return Err(WebhookError::Signature);
	}

	let matches = signatures.iter().any(|signature| {
		let Some(expected) = decode_hex(signature) else {
			return false;
		};
		let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
			.expect("HMAC accepts keys of any length");
		mac.update(timestamp.to_string().as_bytes());
		mac.update(b".");
		mac.update(body);
		mac.verify_slice(&expected).is_ok()
	});

	if matches {
		Ok(())
	} else {
		Err(WebhookError::Signature)
	}
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
	if !text.len().is_multiple_of(2) {
		return None;
	}

	(0..text.len())
		.step_by(2)
		.map(|index| u8::from_str_radix(text.get(index..index + 2)?, 16).ok())
		.collect()
}

/// What a webhook event asks the server to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
	/// Apply this subscription's current state.
	Sync(&'a str),
	/// Create the annual subscription this setup-mode Checkout Session saved
	/// a payment method for.
	CompleteAnnualCheckout(&'a str),
}

/// What `event` asks for, if it is one this endpoint acts on.
pub fn action(event: &Json) -> Option<Action<'_>> {
	let kind = event.get("type")?.as_str()?;
	let object = event.get("data")?.get("object")?;
	let field = |name: &str| object.get(name)?.as_str();

	match kind {
		"customer.subscription.created"
		| "customer.subscription.updated"
		| "customer.subscription.deleted"
		| "customer.subscription.paused"
		| "customer.subscription.resumed" => field("id").map(Action::Sync),
		"checkout.session.completed" if field("mode") == Some("setup") => {
			field("id").map(Action::CompleteAnnualCheckout)
		}
		"checkout.session.completed" => field("subscription").map(Action::Sync),
		"invoice.paid" | "invoice.payment_failed" => {
			object
				.get("parent")
				.and_then(|parent| parent.get("subscription_details"))
				.and_then(|details| details.get("subscription"))
				.or_else(|| object.get("subscription"))?
				.as_str()
				.map(Action::Sync)
		}
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;
	use crate::sync::webhooks::hex;

	const SECRET: &str = "whsec_test";

	fn signed(body: &str, timestamp: u64) -> String {
		let mut mac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).expect("any key");
		mac.update(format!("{timestamp}.{body}").as_bytes());
		format!("t={timestamp},v1={}", hex(&mac.finalize().into_bytes()))
	}

	#[test]
	fn signatures_verify_within_the_tolerance() {
		let body = r#"{"type":"ping"}"#;
		assert_eq!(
			verify(SECRET, &signed(body, 1_000), body.as_bytes(), 1_100),
			Ok(())
		);
		assert_eq!(
			verify(SECRET, &signed(body, 1_000), body.as_bytes(), 2_000),
			Err(WebhookError::Signature),
			"stale events are replays"
		);
		assert_eq!(
			verify("whsec_other", &signed(body, 1_000), body.as_bytes(), 1_000),
			Err(WebhookError::Signature)
		);
		assert_eq!(
			verify(SECRET, "garbage", body.as_bytes(), 1_000),
			Err(WebhookError::Signature)
		);
	}

	#[test]
	fn events_name_what_to_do() {
		let event =
			|kind: &str, object: Json| json!({ "type": kind, "data": { "object": object } });

		assert_eq!(
			action(&event(
				"customer.subscription.updated",
				json!({ "id": "sub_1" })
			)),
			Some(Action::Sync("sub_1"))
		);
		assert_eq!(
			action(&event(
				"checkout.session.completed",
				json!({ "id": "cs_1", "mode": "subscription", "subscription": "sub_2" })
			)),
			Some(Action::Sync("sub_2"))
		);
		assert_eq!(
			action(&event(
				"checkout.session.completed",
				json!({ "id": "cs_2", "mode": "setup", "subscription": null })
			)),
			Some(Action::CompleteAnnualCheckout("cs_2")),
			"an annual checkout saves a card for a subscription the server creates"
		);
		assert_eq!(
			action(&event(
				"invoice.payment_failed",
				json!({ "parent": { "subscription_details": { "subscription": "sub_3" } } })
			)),
			Some(Action::Sync("sub_3"))
		);
		assert_eq!(
			action(&event("invoice.paid", json!({ "subscription": "sub_4" }))),
			Some(Action::Sync("sub_4")),
			"older API versions put the subscription on the invoice"
		);
		assert_eq!(
			action(&event(
				"checkout.session.completed",
				json!({ "mode": "payment", "subscription": null })
			)),
			None,
			"a payment-mode session has no subscription"
		);
		assert_eq!(
			action(&event("charge.refunded", json!({ "id": "ch_1" }))),
			None
		);
	}
}
