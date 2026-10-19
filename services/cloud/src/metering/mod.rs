//! Monthly active documents: counting them, capping free plans, and
//! reporting paid usage to Stripe.
//!
//! A document is **active** in a month once a sync session opens it or the
//! REST API writes it. Each project's active documents are counted per
//! environment and month in the `usage` table.
//!
//! On the free plan, a new document beyond the cap is **paused**: its
//! session stays connected and the client keeps editing locally, but
//! nothing syncs until the next month or an upgrade. Documents already
//! active keep working. Paid plans never pause; going past the included
//! documents is logged, shown by the usage endpoint, and billed through
//! Stripe.

mod plan;
pub mod stripe;

pub use plan::InvalidMonth;
pub use plan::Month;
pub use plan::Plan;
pub use plan::Terms;
pub use plan::UnknownPlan;
use stripe::StripeMeter;

use crate::names::DocumentKey;
use crate::store::Admit;
use crate::store::Store;
use crate::store::StoreError;

/// Whether a document may sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
	/// It may.
	Admitted,
	/// The free plan is full and the document is new this month.
	Paused,
}

/// Counts documents and enforces caps.
#[derive(Clone, Debug)]
pub struct Meter {
	store: Store,
	stripe: Option<StripeMeter>,
}

/// Share of the included documents at which usage reports a warning.
const WARN_AT_PERCENT: u64 = 80;

impl Meter {
	/// A meter over `store`, reporting to Stripe when `stripe` is set.
	pub fn new(store: Store, stripe: Option<StripeMeter>) -> Self {
		Self { store, stripe }
	}

	/// Admit `key` for `month`.
	pub async fn admit(&self, key: &DocumentKey, month: Month) -> Result<Admission, StoreError> {
		let admit = self.store.admit(key, month).await?;

		if admit.new {
			self.counted(key, month, &admit);
		}

		if admit.admitted {
			Ok(Admission::Admitted)
		} else {
			tracing::info!(document = %key, %month, "free plan full; pausing a new document");
			Ok(Admission::Paused)
		}
	}

	fn counted(&self, key: &DocumentKey, month: Month, admit: &Admit) {
		let included = admit.terms.included_documents;

		if let (Some(active), Some(included)) = (admit.active, included)
			&& active > included
		{
			tracing::warn!(project = %key.project, %month, active, included, "over the plan's included documents");
		}

		if let (Some(stripe), Some(customer)) = (&self.stripe, &admit.terms.stripe_customer) {
			stripe.record(customer, key, month);
		}
	}
}

/// How a project's month compares with its plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
	/// Comfortably within the plan.
	Ok,
	/// At 80% of the included documents or more.
	Approaching,
	/// Past the included documents; billed as overage.
	Over,
	/// A free plan at its cap: new documents are paused.
	Capped,
}

impl Standing {
	/// The standing of `active` documents under `terms`.
	pub fn of(terms: &Terms, active: u64) -> Self {
		let Some(included) = terms.included_documents else {
			return Self::Ok;
		};

		if terms.is_full(active) {
			Self::Capped
		} else if active > included {
			Self::Over
		} else if active * 100 >= included * WARN_AT_PERCENT {
			Self::Approaching
		} else {
			Self::Ok
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn standings_follow_the_plan() {
		let hobby = Terms::of(Plan::Hobby);
		assert_eq!(Standing::of(&hobby, 10), Standing::Ok);
		assert_eq!(Standing::of(&hobby, 800), Standing::Approaching);
		assert_eq!(Standing::of(&hobby, 1_000), Standing::Capped);

		let builder = Terms::of(Plan::Builder);
		assert_eq!(Standing::of(&builder, 10_001), Standing::Over);
		assert_eq!(
			Standing::of(&Terms::of(Plan::Enterprise), 1 << 40),
			Standing::Ok
		);
	}
}
