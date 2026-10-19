//! Subscriptions: starting one with Checkout, switching plans, opening the
//! Customer Portal, and turning a subscription's state into project terms.
//!
//! A subscription names its project in `metadata.canvist_project` and its
//! plan through its prices' lookup keys (`canvist_builder_…`,
//! `canvist_scale_…`), as `services/cloud/stripe/setup.ts` creates them.
//! Every subscription is anchored to the first of the month, so Stripe's
//! billing periods match the calendar months monthly active documents are
//! counted in.
//!
//! Losing a paid plan never makes documents read-only: the project returns
//! to Hobby, documents already active this month keep syncing, and new ones
//! past the free cap pause sync while clients keep editing locally.

use serde_json::Value as Json;

use super::Month;
use super::Plan;
use super::Subscription;
use super::SubscriptionStatus;
use super::Terms;
use super::stripe::StripeClient;
use super::stripe::StripeError;
use crate::names::ProjectId;
use crate::store::Store;
use crate::store::StoreError;

/// Tags Checkout Sessions so the Dashboard can tell this flow apart.
const INTEGRATION_IDENTIFIER: &str = "canvist-cloud-checkout-qhzvmbte";
/// The metadata key that names a subscription's project.
const PROJECT_METADATA: &str = "canvist_project";
/// The prices a Checkout subscription is made of, after `canvist_<plan>_`.
const SUBSCRIPTION_PRICES: [&str; 3] = ["monthly", "mad_overage", "conversion_overage"];

/// Why a billing request failed.
#[derive(Debug, thiserror::Error)]
pub enum BillingError {
	/// Stripe is not configured on this instance.
	#[error("billing is not configured on this server")]
	NotConfigured,
	/// The project already has a subscription that pays.
	#[error(
		"the project already has a subscription; switch plans or open the billing portal instead"
	)]
	AlreadySubscribed,
	/// The project has no subscription that pays.
	#[error("the project has no paid subscription; start one with checkout")]
	NotSubscribed,
	/// The project has never had a Stripe customer.
	#[error("the project has no billing account yet; start a subscription with checkout")]
	NoCustomer,
	/// Only Builder and Scale are sold self-serve.
	#[error("only the builder and scale plans can be bought; contact sales for enterprise")]
	NotSelfServe,
	/// A price the catalog should have is missing.
	#[error("the Stripe price `{0}` does not exist; run the billing setup")]
	MissingPrice(String),
	/// The Customer Portal has no canvist configuration.
	#[error("the billing portal is not configured; run the billing setup")]
	MissingPortal,
	/// Stripe sent something this server does not understand.
	#[error("unexpected Stripe response: {0}")]
	Malformed(&'static str),
	/// Stripe failed.
	#[error(transparent)]
	Stripe(#[from] StripeError),
	/// The store failed.
	#[error(transparent)]
	Store(#[from] StoreError),
}

/// What a checkout needs from the caller.
#[derive(Debug, serde::Deserialize)]
pub struct CheckoutRequest {
	/// `builder` or `scale`.
	pub plan: Plan,
	/// Where Stripe sends the customer after paying.
	pub success_url: String,
	/// Where Stripe sends the customer if they go back.
	pub cancel_url: String,
}

/// Start a monthly subscription to `request.plan` for `project`, returning
/// the Checkout Session's URL.
///
/// Annual plans are not sold here: an annual fee with monthly metered
/// overage is a mixed-interval subscription, which Checkout cannot create.
pub async fn checkout(
	stripe: &StripeClient,
	store: &Store,
	project: &ProjectId,
	request: &CheckoutRequest,
) -> Result<String, BillingError> {
	let plan = self_serve(request.plan)?;
	let terms = store.terms(project).await?;

	if terms.is_subscribed() {
		return Err(BillingError::AlreadySubscribed);
	}

	let keys = SUBSCRIPTION_PRICES.map(|suffix| lookup_key(plan, suffix));
	let prices = prices(stripe, &keys).await?;
	let anchor = Month::current().next().start_unix_seconds();
	let mut form = form(&[
		("mode", "subscription"),
		("client_reference_id", project.as_str()),
		("success_url", &request.success_url),
		("cancel_url", &request.cancel_url),
		("metadata[canvist_project]", project.as_str()),
		(
			"subscription_data[metadata][canvist_project]",
			project.as_str(),
		),
		(
			"subscription_data[billing_cycle_anchor]",
			&anchor.to_string(),
		),
		("tax_id_collection[enabled]", "true"),
		("integration_identifier", INTEGRATION_IDENTIFIER),
	]);

	for (index, price) in prices.iter().enumerate() {
		form.push((format!("line_items[{index}][price]"), price.id.clone()));

		// Metered prices take no quantity.
		if !price.metered {
			form.push((format!("line_items[{index}][quantity]"), "1".to_owned()));
		}
	}

	if stripe.config().automatic_tax {
		form.push(("automatic_tax[enabled]".to_owned(), "true".to_owned()));
	}

	if let Some(customer) = &terms.stripe_customer {
		form.extend(self::form(&[
			("customer", customer),
			// Tax IDs and the address entered at checkout update the
			// returning customer, so invoices and tax use them.
			("customer_update[name]", "auto"),
			("customer_update[address]", "auto"),
		]));
	}

	let session = stripe.post("/v1/checkout/sessions", &form).await?;

	text(&session, "url").ok_or(BillingError::Malformed("a checkout session without a url"))
}

/// Open the Customer Portal for `project`'s customer, returning its URL.
pub async fn portal(
	stripe: &StripeClient,
	store: &Store,
	project: &ProjectId,
	return_url: &str,
) -> Result<String, BillingError> {
	let terms = store.terms(project).await?;
	let customer = terms.stripe_customer.ok_or(BillingError::NoCustomer)?;
	let configurations = stripe
		.get(
			"/v1/billing_portal/configurations",
			&[("active", "true"), ("limit", "100")],
		)
		.await?;
	let configuration = list(&configurations)
		.iter()
		.find(|configuration| {
			configuration
				.get("metadata")
				.and_then(|metadata| text(metadata, "canvist_key"))
				.as_deref()
				== Some("portal")
		})
		.and_then(|configuration| text(configuration, "id"))
		.ok_or(BillingError::MissingPortal)?;
	let session = stripe
		.post(
			"/v1/billing_portal/sessions",
			&form(&[
				("customer", &customer),
				("return_url", return_url),
				("configuration", &configuration),
			]),
		)
		.await?;

	text(&session, "url").ok_or(BillingError::Malformed("a portal session without a url"))
}

/// Switch `project`'s subscription to `plan`, swapping each canvist price
/// for the same price of the new plan. The base fee is prorated; metered
/// usage this period is billed at the new plan's rates.
pub async fn change_plan(
	stripe: &StripeClient,
	store: &Store,
	project: &ProjectId,
	plan: Plan,
) -> Result<Terms, BillingError> {
	let plan = self_serve(plan)?;
	let terms = store.terms(project).await?;
	let subscription = terms
		.subscription
		.as_ref()
		.filter(|subscription| subscription.status.grants_plan())
		.ok_or(BillingError::NotSubscribed)?;

	if terms.plan == plan {
		return Ok(terms);
	}

	let current = stripe
		.get(&format!("/v1/subscriptions/{}", subscription.id), &[])
		.await?;
	let swaps: Vec<(String, String)> = items(&current)
		.iter()
		.filter_map(|item| {
			let key = item
				.get("price")
				.and_then(|price| text(price, "lookup_key"))?;
			let (_, suffix) = key.strip_prefix("canvist_")?.split_once('_')?;
			Some((text(item, "id")?, lookup_key(plan, suffix)))
		})
		.collect();
	let keys: Vec<String> = swaps.iter().map(|(_, key)| key.clone()).collect();
	let prices = prices(stripe, &keys).await?;
	let mut form = form(&[("proration_behavior", "create_prorations")]);

	for (index, ((item, _), price)) in swaps.iter().zip(&prices).enumerate() {
		form.push((format!("items[{index}][id]"), item.clone()));
		form.push((format!("items[{index}][price]"), price.id.clone()));
	}

	let updated = stripe
		.post(&format!("/v1/subscriptions/{}", subscription.id), &form)
		.await?;

	apply(store, &updated)
		.await?
		.map(|(_, terms)| terms)
		.ok_or(BillingError::Malformed(
			"the updated subscription has no canvist plan",
		))
}

/// Retrieve subscription `id` and apply its current state to its project.
/// Returns the project and its new terms, or `None` when the subscription is
/// not canvist's or no longer concerns the project.
pub async fn sync_subscription(
	stripe: &StripeClient,
	store: &Store,
	id: &str,
) -> Result<Option<(ProjectId, Terms)>, BillingError> {
	let subscription = stripe.get(&format!("/v1/subscriptions/{id}"), &[]).await?;
	apply(store, &subscription).await
}

/// Apply `subscription`'s state to the project it names.
async fn apply(
	store: &Store,
	subscription: &Json,
) -> Result<Option<(ProjectId, Terms)>, BillingError> {
	let Some(project) = subscription
		.get("metadata")
		.and_then(|metadata| text(metadata, PROJECT_METADATA))
		.map(ProjectId::new)
	else {
		return Ok(None);
	};
	let current = match store.terms(&project).await {
		Ok(current) => current,
		Err(StoreError::UnknownProject) => {
			tracing::warn!(%project, "a Stripe subscription names an unknown project");
			return Ok(None);
		}
		Err(error) => return Err(error.into()),
	};
	let Some(terms) = terms_after(&current, subscription)? else {
		return Ok(None);
	};

	if terms != current {
		store.set_terms(&project, &terms).await?;
		tracing::info!(%project, plan = terms.plan.as_str(), status = ?terms.subscription.as_ref().map(|subscription| subscription.status), "billing changed by Stripe");
	}

	Ok(Some((project, terms)))
}

/// The terms a project with `current` terms has under `subscription`, or
/// `None` when the subscription does not concern it: not a canvist plan, or
/// an old subscription that no longer pays while another one does.
fn terms_after(current: &Terms, subscription: &Json) -> Result<Option<Terms>, BillingError> {
	let id =
		text(subscription, "id").ok_or(BillingError::Malformed("a subscription without an id"))?;
	let customer = text(subscription, "customer")
		.ok_or(BillingError::Malformed("a subscription without a customer"))?;
	let status: SubscriptionStatus = text(subscription, "status")
		.and_then(|status| status.parse().ok())
		.ok_or(BillingError::Malformed(
			"a subscription with an unknown status",
		))?;
	let Some(plan) = subscribed_plan(subscription) else {
		return Ok(None);
	};
	let replaced = current
		.subscription
		.as_ref()
		.is_some_and(|known| known.id != id);

	if replaced && !status.grants_plan() {
		return Ok(None);
	}

	let paid = if status.grants_plan() {
		Terms::of(plan)
	} else if status == SubscriptionStatus::Incomplete && current.subscription.is_none() {
		// Checkout has not finished paying; keep whatever the project had.
		current.clone()
	} else {
		Terms::of(Plan::Hobby)
	};

	Ok(Some(Terms {
		stripe_customer: Some(customer),
		subscription: Some(Subscription { id, status }),
		..paid
	}))
}

/// The plan a subscription's prices name through their lookup keys.
fn subscribed_plan(subscription: &Json) -> Option<Plan> {
	items(subscription)
		.iter()
		.filter_map(|item| {
			item.get("price")
				.and_then(|price| text(price, "lookup_key"))
		})
		.find_map(|key| plan_of_lookup_key(&key))
}

fn plan_of_lookup_key(key: &str) -> Option<Plan> {
	let (plan, _) = key.strip_prefix("canvist_")?.split_once('_')?;
	self_serve(plan.parse().ok()?).ok()
}

/// `plan` if it is sold self-serve.
fn self_serve(plan: Plan) -> Result<Plan, BillingError> {
	match plan {
		Plan::Builder | Plan::Scale => Ok(plan),
		Plan::Hobby | Plan::Enterprise => Err(BillingError::NotSelfServe),
	}
}

fn lookup_key(plan: Plan, suffix: &str) -> String {
	format!("canvist_{}_{suffix}", plan.as_str())
}

/// An active price, found by lookup key.
#[derive(Debug)]
struct Price {
	id: String,
	metered: bool,
}

/// The active prices with `keys`, in the same order.
async fn prices(stripe: &StripeClient, keys: &[String]) -> Result<Vec<Price>, BillingError> {
	let mut query = vec![("active", "true"), ("limit", "100")];
	query.extend(keys.iter().map(|key| ("lookup_keys[]", key.as_str())));
	let found = stripe.get("/v1/prices", &query).await?;
	let found = list(&found);

	keys.iter()
		.map(|key| {
			let price = found
				.iter()
				.find(|price| text(price, "lookup_key").as_deref() == Some(key))
				.ok_or_else(|| BillingError::MissingPrice(key.clone()))?;
			let metered = price
				.get("recurring")
				.and_then(|recurring| text(recurring, "usage_type"))
				.as_deref()
				== Some("metered");

			Ok(Price {
				id: text(price, "id").ok_or(BillingError::Malformed("a price without an id"))?,
				metered,
			})
		})
		.collect()
}

fn form(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
	pairs
		.iter()
		.map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
		.collect()
}

fn text(value: &Json, field: &str) -> Option<String> {
	value.get(field)?.as_str().map(str::to_owned)
}

fn list(value: &Json) -> &[Json] {
	value
		.get("data")
		.and_then(Json::as_array)
		.map_or(&[], Vec::as_slice)
}

fn items(subscription: &Json) -> &[Json] {
	subscription.get("items").map_or(&[], list)
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	fn subscription(id: &str, status: &str, lookup_key: &str) -> Json {
		json!({
			"id": id,
			"customer": "cus_1",
			"status": status,
			"metadata": { "canvist_project": "prj_a" },
			"items": { "data": [
				{ "id": "si_1", "price": { "lookup_key": lookup_key } },
				{ "id": "si_2", "price": { "lookup_key": "canvist_builder_mad_overage" } },
			] },
		})
	}

	fn on(plan: Plan, id: &str, status: SubscriptionStatus) -> Terms {
		Terms {
			stripe_customer: Some("cus_1".to_owned()),
			subscription: Some(Subscription {
				id: id.to_owned(),
				status,
			}),
			..Terms::of(plan)
		}
	}

	#[test]
	fn paying_subscriptions_grant_their_plan() {
		let hobby = Terms::of(Plan::Hobby);

		for status in ["active", "trialing", "past_due"] {
			let terms = terms_after(
				&hobby,
				&subscription("sub_1", status, "canvist_scale_monthly"),
			)
			.expect("valid")
			.expect("concerns the project");
			assert_eq!(terms.plan, Plan::Scale, "{status}");
			assert_eq!(terms.included_documents, Some(150_000));
			assert_eq!(terms.stripe_customer.as_deref(), Some("cus_1"));
		}
	}

	#[test]
	fn ended_subscriptions_return_to_hobby_and_keep_the_customer() {
		let builder = on(Plan::Builder, "sub_1", SubscriptionStatus::Active);

		for status in ["unpaid", "canceled", "paused", "incomplete_expired"] {
			let terms = terms_after(
				&builder,
				&subscription("sub_1", status, "canvist_builder_monthly"),
			)
			.expect("valid")
			.expect("concerns the project");
			assert_eq!(terms.plan, Plan::Hobby, "{status}");
			assert_eq!(terms.stripe_customer.as_deref(), Some("cus_1"));
			assert_eq!(terms.metered_customer(), None, "Hobby is never metered");
		}
	}

	#[test]
	fn an_unfinished_checkout_changes_nothing() {
		let hobby = Terms::of(Plan::Hobby);
		let terms = terms_after(
			&hobby,
			&subscription("sub_1", "incomplete", "canvist_builder_monthly"),
		)
		.expect("valid")
		.expect("concerns the project");
		assert_eq!(terms.plan, Plan::Hobby);
		assert!(!terms.is_subscribed());
	}

	#[test]
	fn an_old_subscription_cannot_downgrade_its_replacement() {
		let scale = on(Plan::Scale, "sub_new", SubscriptionStatus::Active);
		assert!(
			terms_after(
				&scale,
				&subscription("sub_old", "canceled", "canvist_builder_monthly")
			)
			.expect("valid")
			.is_none()
		);

		let adopted = terms_after(
			&scale,
			&subscription("sub_newer", "active", "canvist_builder_monthly"),
		)
		.expect("valid")
		.expect("a paying subscription is adopted");
		assert_eq!(adopted.plan, Plan::Builder);
	}

	#[test]
	fn foreign_and_free_prices_are_ignored() {
		let hobby = Terms::of(Plan::Hobby);
		let mut foreign = subscription("sub_1", "active", "acme_pro");
		foreign["items"]["data"][1]["price"]["lookup_key"] = json!("acme_extra");
		assert!(terms_after(&hobby, &foreign).expect("valid").is_none());
		assert_eq!(
			plan_of_lookup_key("canvist_scale_annual"),
			Some(Plan::Scale)
		);
		assert_eq!(plan_of_lookup_key("canvist_hobby_monthly"), None);
		assert_eq!(plan_of_lookup_key("canvist_enterprise_monthly"), None);
	}

	#[test]
	fn malformed_subscriptions_are_errors() {
		let mut unknown = subscription("sub_1", "active", "canvist_builder_monthly");
		unknown["status"] = json!("exploded");
		assert!(terms_after(&Terms::of(Plan::Hobby), &unknown).is_err());
	}
}
