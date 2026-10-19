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
//! Monthly plans are sold by Checkout in `subscription` mode. An annual plan
//! bills its base fee yearly and its overage monthly, a mixed-interval
//! subscription that Checkout cannot create, so Checkout runs in `setup`
//! mode to save a payment method and the webhook creates the subscription
//! ([`complete_annual_checkout`]).
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
/// The metadata key that names the plan an annual Checkout Session sells.
const PLAN_METADATA: &str = "canvist_plan";
/// The metadata key that names the Checkout Session an annual subscription
/// was created for, so a retried webhook finds it instead of creating
/// another.
const CHECKOUT_SESSION_METADATA: &str = "canvist_checkout_session";
/// The overage prices every subscription has, after `canvist_<plan>_`.
const OVERAGE_PRICES: [&str; 2] = ["mad_overage", "conversion_overage"];

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

/// How often a plan's base fee is billed. Overage is billed monthly either
/// way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
	/// Monthly.
	#[default]
	Month,
	/// Yearly, at the annual price.
	Year,
}

impl Interval {
	/// The base price's lookup key suffix, after `canvist_<plan>_`.
	fn base_price(self) -> &'static str {
		match self {
			Self::Month => "monthly",
			Self::Year => "annual",
		}
	}
}

/// What a checkout needs from the caller.
#[derive(Debug, serde::Deserialize)]
pub struct CheckoutRequest {
	/// `builder` or `scale`.
	pub plan: Plan,
	/// `month` (the default) or `year`.
	#[serde(default)]
	pub interval: Interval,
	/// Where Stripe sends the customer after paying.
	pub success_url: String,
	/// Where Stripe sends the customer if they go back.
	pub cancel_url: String,
}

/// Start a subscription to `request.plan` for `project`, returning the
/// Checkout Session's URL.
///
/// A monthly plan is a `subscription` mode session. An annual plan is a
/// `setup` mode session that saves a payment method; the subscription is
/// created when it completes ([`complete_annual_checkout`]).
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

	// Looked up for annual plans too, so a missing price fails before the
	// customer enters a card.
	let prices = plan_prices(stripe, plan, request.interval).await?;
	let mut form = form(&[
		("client_reference_id", project.as_str()),
		("success_url", &request.success_url),
		("cancel_url", &request.cancel_url),
		("metadata[canvist_project]", project.as_str()),
		("tax_id_collection[enabled]", "true"),
		("integration_identifier", INTEGRATION_IDENTIFIER),
	]);

	match request.interval {
		Interval::Month => {
			form.extend(self::form(&[
				("mode", "subscription"),
				(
					"subscription_data[metadata][canvist_project]",
					project.as_str(),
				),
				("subscription_data[billing_cycle_anchor]", &next_anchor()),
			]));
			form.extend(items("line_items", &prices));

			if stripe.config().automatic_tax {
				form.push(("automatic_tax[enabled]".to_owned(), "true".to_owned()));
			}
		}
		Interval::Year => {
			let base = prices
				.first()
				.ok_or(BillingError::Malformed("a plan without prices"))?;
			form.extend(self::form(&[
				("mode", "setup"),
				("currency", &base.currency),
				("metadata[canvist_plan]", plan.as_str()),
			]));

			// The subscription is taxed when it is created, from the address
			// saved here.
			if stripe.config().automatic_tax {
				form.push((
					"billing_address_collection".to_owned(),
					"required".to_owned(),
				));
			}

			if terms.stripe_customer.is_none() {
				form.push(("customer_creation".to_owned(), "always".to_owned()));
			}
		}
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
/// for the same price of the new plan, so an annual subscription stays
/// annual. The base fee is prorated. Metered usage before the switch is
/// billed at the old plan's rates; the new plan's tiers, included documents
/// and all, start from the switch.
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
	let swaps: Vec<(String, String)> = subscription_items(&current)
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

/// Create the annual subscription that completed setup-mode Checkout
/// Session `id` was for, then apply it as [`sync_subscription`] does.
///
/// The subscription bills the plan's annual base fee and both monthly
/// overage prices in flexible billing mode, anchored to the first of next
/// month and charged to the payment method the session saved. Creating it is
/// safe to repeat: the request's idempotency key comes from the session, and
/// a subscription already created for the session is found and applied
/// instead, after Stripe has forgotten the key. Returns `None` when the
/// session is not a finished canvist annual checkout, or its project already
/// pays through another subscription.
pub async fn complete_annual_checkout(
	stripe: &StripeClient,
	store: &Store,
	id: &str,
) -> Result<Option<(ProjectId, Terms)>, BillingError> {
	let session = stripe
		.get(
			&format!("/v1/checkout/sessions/{id}"),
			&[("expand[]", "setup_intent")],
		)
		.await?;
	let Some(order) = AnnualOrder::of(&session)? else {
		return Ok(None);
	};
	let Some(terms) = known_terms(store, &order.project).await? else {
		return Ok(None);
	};
	let existing = stripe
		.get(
			"/v1/subscriptions",
			&[
				("customer", &order.customer),
				("status", "all"),
				("limit", "100"),
			],
		)
		.await?;

	if let Some(subscription) = list(&existing).iter().find(|subscription| {
		subscription
			.get("metadata")
			.and_then(|metadata| text(metadata, CHECKOUT_SESSION_METADATA))
			.as_deref()
			== Some(id)
	}) {
		return apply(store, subscription).await;
	}

	if terms.is_subscribed() {
		tracing::warn!(project = %order.project, session = id, "an annual checkout finished for a project that already pays; not starting a second subscription");
		return Ok(None);
	}

	let prices = plan_prices(stripe, order.plan, Interval::Year).await?;
	let mut form = form(&[
		("customer", &order.customer),
		("default_payment_method", &order.payment_method),
		("billing_mode[type]", "flexible"),
		("billing_cycle_anchor", &next_anchor()),
		// The customer saved the card for this and is no longer on the page.
		("off_session", "true"),
		("metadata[canvist_project]", order.project.as_str()),
		("metadata[canvist_checkout_session]", id),
	]);
	form.extend(items("items", &prices));

	if stripe.config().automatic_tax {
		form.push(("automatic_tax[enabled]".to_owned(), "true".to_owned()));
	}

	let created = stripe
		.post_once("/v1/subscriptions", &form, &format!("canvist-annual-{id}"))
		.await?;

	apply(store, &created).await
}

/// What a completed setup-mode Checkout Session ordered.
#[derive(Debug, PartialEq, Eq)]
struct AnnualOrder {
	project: ProjectId,
	plan: Plan,
	customer: String,
	payment_method: String,
}

impl AnnualOrder {
	/// The order in `session`, retrieved with its setup intent expanded, or
	/// `None` when it is not a finished canvist annual checkout.
	fn of(session: &Json) -> Result<Option<Self>, BillingError> {
		let metadata = session.get("metadata");
		let field = |name: &str| metadata.and_then(|metadata| text(metadata, name));
		let (Some(project), Some(plan)) = (field(PROJECT_METADATA), field(PLAN_METADATA)) else {
			return Ok(None);
		};

		if text(session, "mode").as_deref() != Some("setup")
			|| text(session, "status").as_deref() != Some("complete")
		{
			return Ok(None);
		}

		let plan = plan
			.parse()
			.ok()
			.and_then(|plan| self_serve(plan).ok())
			.ok_or(BillingError::Malformed(
				"an annual checkout for a plan that is not sold",
			))?;
		let customer = text(session, "customer").ok_or(BillingError::Malformed(
			"an annual checkout without a customer",
		))?;
		let payment_method = session
			.get("setup_intent")
			.and_then(|intent| text(intent, "payment_method"))
			.ok_or(BillingError::Malformed(
				"an annual checkout without a payment method",
			))?;

		Ok(Some(Self {
			project: ProjectId::new(project),
			plan,
			customer,
			payment_method,
		}))
	}
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
	let Some(current) = known_terms(store, &project).await? else {
		return Ok(None);
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

/// `project`'s terms, or `None` when Stripe names a project this server does
/// not know.
async fn known_terms(store: &Store, project: &ProjectId) -> Result<Option<Terms>, BillingError> {
	match store.terms(project).await {
		Ok(terms) => Ok(Some(terms)),
		Err(StoreError::UnknownProject) => {
			tracing::warn!(%project, "Stripe names an unknown project");
			Ok(None)
		}
		Err(error) => Err(error.into()),
	}
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
	subscription_items(subscription)
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

/// 00:00 UTC on the first of next month, where subscriptions are anchored.
fn next_anchor() -> String {
	Month::current().next().start_unix_seconds().to_string()
}

/// An active price, found by lookup key.
#[derive(Debug)]
struct Price {
	id: String,
	currency: String,
	metered: bool,
}

/// The base price for `interval` and the overage prices of `plan`, in that
/// order.
async fn plan_prices(
	stripe: &StripeClient,
	plan: Plan,
	interval: Interval,
) -> Result<Vec<Price>, BillingError> {
	let keys: Vec<String> = std::iter::once(interval.base_price())
		.chain(OVERAGE_PRICES)
		.map(|suffix| lookup_key(plan, suffix))
		.collect();
	prices(stripe, &keys).await
}

/// Form fields `<field>[i][price]` for each of `prices`, with a quantity of
/// one for licensed prices. Metered prices take no quantity.
fn items(field: &str, prices: &[Price]) -> Vec<(String, String)> {
	let mut form = Vec::new();

	for (index, price) in prices.iter().enumerate() {
		form.push((format!("{field}[{index}][price]"), price.id.clone()));

		if !price.metered {
			form.push((format!("{field}[{index}][quantity]"), "1".to_owned()));
		}
	}

	form
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
				currency: text(price, "currency")
					.ok_or(BillingError::Malformed("a price without a currency"))?,
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

fn subscription_items(subscription: &Json) -> &[Json] {
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

	fn setup_session(status: &str) -> Json {
		json!({
			"id": "cs_1",
			"mode": "setup",
			"status": status,
			"customer": "cus_1",
			"metadata": { "canvist_project": "prj_a", "canvist_plan": "scale" },
			"setup_intent": { "id": "seti_1", "payment_method": "pm_1" },
		})
	}

	#[test]
	fn completed_annual_checkouts_name_their_order() {
		assert_eq!(
			AnnualOrder::of(&setup_session("complete")).expect("valid"),
			Some(AnnualOrder {
				project: ProjectId::new("prj_a"),
				plan: Plan::Scale,
				customer: "cus_1".to_owned(),
				payment_method: "pm_1".to_owned(),
			})
		);
		assert_eq!(
			AnnualOrder::of(&setup_session("open")).expect("valid"),
			None,
			"an unfinished session orders nothing"
		);

		let mut foreign = setup_session("complete");
		foreign["metadata"] = json!({});
		assert_eq!(AnnualOrder::of(&foreign).expect("valid"), None);

		let mut enterprise = setup_session("complete");
		enterprise["metadata"]["canvist_plan"] = json!("enterprise");
		assert!(AnnualOrder::of(&enterprise).is_err());

		let mut unexpanded = setup_session("complete");
		unexpanded["setup_intent"] = json!("seti_1");
		assert!(
			AnnualOrder::of(&unexpanded).is_err(),
			"the payment method needs the setup intent expanded"
		);
	}

	#[test]
	fn malformed_subscriptions_are_errors() {
		let mut unknown = subscription("sub_1", "active", "canvist_builder_monthly");
		unknown["status"] = json!("exploded");
		assert!(terms_after(&Terms::of(Plan::Hobby), &unknown).is_err());
	}
}
