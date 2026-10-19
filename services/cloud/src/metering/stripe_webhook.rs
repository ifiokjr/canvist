//! Stripe subscription webhooks: a subscription change sets the project's
//! plan, so an upgrade lifts a free plan's cap without anyone calling the
//! admin API.
//!
//! The subscription names its project in `metadata.canvist_project`, and
//! its plan through the lookup key of one of its prices (`canvist_builder_…`
//! or `canvist_scale_…`, as `services/cloud/stripe/setup.ts` creates them).
//! An active, trialing, or past-due subscription grants its plan; any other
//! status, or a deleted subscription, returns the project to Hobby.

use hmac::Hmac;
use hmac::Mac;
use serde_json::Value as Json;
use sha2::Sha256;

use super::Plan;
use super::Terms;
use crate::names::ProjectId;

/// How old a signed event may be, in seconds, as Stripe's libraries allow.
pub const TOLERANCE_SECONDS: u64 = 300;

/// The events this endpoint acts on.
pub const EVENTS: [&str; 3] = [
	"customer.subscription.created",
	"customer.subscription.updated",
	"customer.subscription.deleted",
];

/// Subscription statuses that keep a paid plan. `past_due` keeps it while
/// Stripe retries the payment.
const PAYING: [&str; 3] = ["active", "trialing", "past_due"];

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

/// A project's new terms, from a subscription event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanChange {
	/// The project the subscription pays for.
	pub project: ProjectId,
	/// Its terms from now on.
	pub terms: Terms,
}

/// The plan change `event` asks for, if it is a subscription event for a
/// canvist project with a canvist price.
pub fn plan_change(event: &Json) -> Option<PlanChange> {
	let kind = event.get("type")?.as_str()?;

	if !EVENTS.contains(&kind) {
		return None;
	}

	let subscription = event.get("data")?.get("object")?;
	let project = subscription
		.get("metadata")?
		.get("canvist_project")?
		.as_str()?;
	let customer = subscription.get("customer")?.as_str()?;
	let plan = subscribed_plan(subscription)?;
	let status = subscription.get("status")?.as_str()?;
	let paying = kind != "customer.subscription.deleted" && PAYING.contains(&status);
	let terms = if paying {
		Terms {
			stripe_customer: Some(customer.to_owned()),
			..Terms::of(plan)
		}
	} else {
		Terms::of(Plan::Hobby)
	};

	Some(PlanChange {
		project: ProjectId::new(project),
		terms,
	})
}

/// The plan a subscription's prices name through their lookup keys.
fn subscribed_plan(subscription: &Json) -> Option<Plan> {
	subscription
		.get("items")?
		.get("data")?
		.as_array()?
		.iter()
		.filter_map(|item| item.get("price")?.get("lookup_key")?.as_str())
		.find_map(plan_of_lookup_key)
}

fn plan_of_lookup_key(key: &str) -> Option<Plan> {
	let rest = key.strip_prefix("canvist_")?;
	let (plan, _) = rest.split_once('_')?;

	match plan.parse().ok()? {
		Plan::Hobby | Plan::Enterprise => None,
		plan => Some(plan),
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

	fn subscription(kind: &str, status: &str, lookup_key: &str) -> Json {
		json!({
			"type": kind,
			"data": { "object": {
				"customer": "cus_1",
				"status": status,
				"metadata": { "canvist_project": "prj_a" },
				"items": { "data": [
					{ "price": { "lookup_key": "canvist_builder_mad_overage" } },
					{ "price": { "lookup_key": lookup_key } },
				] },
			} },
		})
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
	fn active_subscriptions_set_their_plan() {
		let change = plan_change(&subscription(
			"customer.subscription.created",
			"active",
			"canvist_scale_monthly",
		))
		.expect("a plan change");
		assert_eq!(change.project, ProjectId::new("prj_a"));
		assert_eq!(
			change.terms.plan,
			Plan::Builder,
			"the first canvist price wins"
		);
		assert_eq!(change.terms.stripe_customer.as_deref(), Some("cus_1"));
	}

	#[test]
	fn cancelled_subscriptions_return_to_hobby() {
		for (kind, status) in [
			("customer.subscription.deleted", "canceled"),
			("customer.subscription.updated", "unpaid"),
		] {
			let change = plan_change(&subscription(kind, status, "canvist_builder_monthly"))
				.expect("a plan change");
			assert_eq!(change.terms, Terms::of(Plan::Hobby));
		}
	}

	#[test]
	fn other_events_and_foreign_prices_are_ignored() {
		assert_eq!(plan_change(&json!({ "type": "invoice.paid" })), None);
		let mut foreign = subscription("customer.subscription.created", "active", "other_price");
		foreign["data"]["object"]["items"]["data"][0]["price"]["lookup_key"] = json!("acme_pro");
		assert_eq!(plan_change(&foreign), None);
		assert_eq!(
			plan_of_lookup_key("canvist_scale_annual"),
			Some(Plan::Scale)
		);
		assert_eq!(plan_of_lookup_key("canvist_hobby_monthly"), None);
	}
}
