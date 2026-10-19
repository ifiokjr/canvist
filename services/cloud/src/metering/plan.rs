//! Plans and billing months.

use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

/// A canvist Cloud plan. Prices live in `docs/src/v2/business.md`; the
/// server only knows what each plan includes and whether it caps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
	/// Free, with a hard cap.
	Hobby,
	/// Paid; overage is billed, never refused.
	Builder,
	/// Paid, with more included.
	Scale,
	/// A contract; included documents are set per project.
	Enterprise,
}

impl Plan {
	/// Monthly active documents included, if the plan sets a number.
	pub fn included_documents(self) -> Option<u64> {
		match self {
			Self::Hobby => Some(1_000),
			Self::Builder => Some(10_000),
			Self::Scale => Some(150_000),
			Self::Enterprise => None,
		}
	}

	/// Whether reaching the included documents pauses new ones. Only the
	/// free plan caps; paid plans warn and bill the overage.
	pub fn is_capped(self) -> bool {
		matches!(self, Self::Hobby)
	}

	/// The plan's name as stored.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Hobby => "hobby",
			Self::Builder => "builder",
			Self::Scale => "scale",
			Self::Enterprise => "enterprise",
		}
	}
}

impl FromStr for Plan {
	type Err = UnknownPlan;

	fn from_str(name: &str) -> Result<Self, Self::Err> {
		[Self::Hobby, Self::Builder, Self::Scale, Self::Enterprise]
			.into_iter()
			.find(|plan| plan.as_str() == name)
			.ok_or(UnknownPlan)
	}
}

/// A plan name the server does not know.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown plan")]
pub struct UnknownPlan;

/// A project's billing terms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Terms {
	/// The plan.
	pub plan: Plan,
	/// Included documents, after any per-project override.
	pub included_documents: Option<u64>,
	/// The project's Stripe customer. It outlives subscriptions, so a
	/// project that subscribes again keeps one customer.
	#[serde(skip)]
	pub stripe_customer: Option<String>,
	/// The Stripe subscription behind the plan, if there ever was one.
	#[serde(skip)]
	pub subscription: Option<Subscription>,
}

impl Terms {
	/// The default terms of `plan`.
	pub fn of(plan: Plan) -> Self {
		Self {
			plan,
			included_documents: plan.included_documents(),
			stripe_customer: None,
			subscription: None,
		}
	}

	/// The customer to bill usage to: only while a paid plan is in force.
	pub fn metered_customer(&self) -> Option<&str> {
		match self.plan {
			Plan::Hobby => None,
			Plan::Builder | Plan::Scale | Plan::Enterprise => self.stripe_customer.as_deref(),
		}
	}

	/// Whether a subscription currently pays for the plan.
	pub fn is_subscribed(&self) -> bool {
		self.subscription
			.as_ref()
			.is_some_and(|subscription| subscription.status.grants_plan())
	}

	/// Whether `active` documents leave no room for another under a cap.
	pub fn is_full(&self, active: u64) -> bool {
		self.plan.is_capped()
			&& self
				.included_documents
				.is_some_and(|included| active >= included)
	}
}

/// A Stripe subscription and its last known status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription {
	/// `sub_…`.
	pub id: String,
	/// Its status when Stripe last told us.
	pub status: SubscriptionStatus,
}

/// A Stripe subscription status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
	/// Created, waiting for its first payment.
	Incomplete,
	/// The first payment never came.
	IncompleteExpired,
	/// In a free trial.
	Trialing,
	/// Paid up.
	Active,
	/// A payment failed and Stripe is retrying it.
	PastDue,
	/// Ended.
	Canceled,
	/// Retries ran out; the latest invoice is still open.
	Unpaid,
	/// Collection is paused.
	Paused,
}

impl SubscriptionStatus {
	const ALL: [Self; 8] = [
		Self::Incomplete,
		Self::IncompleteExpired,
		Self::Trialing,
		Self::Active,
		Self::PastDue,
		Self::Canceled,
		Self::Unpaid,
		Self::Paused,
	];

	/// Whether the subscription keeps its paid plan. A failed payment keeps
	/// it while Stripe retries, so customers never lose service mid-dunning.
	pub fn grants_plan(self) -> bool {
		matches!(self, Self::Trialing | Self::Active | Self::PastDue)
	}

	/// The status as Stripe writes it.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Incomplete => "incomplete",
			Self::IncompleteExpired => "incomplete_expired",
			Self::Trialing => "trialing",
			Self::Active => "active",
			Self::PastDue => "past_due",
			Self::Canceled => "canceled",
			Self::Unpaid => "unpaid",
			Self::Paused => "paused",
		}
	}
}

impl FromStr for SubscriptionStatus {
	type Err = UnknownStatus;

	fn from_str(name: &str) -> Result<Self, Self::Err> {
		Self::ALL
			.into_iter()
			.find(|status| status.as_str() == name)
			.ok_or(UnknownStatus)
	}
}

/// A subscription status the server does not know.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown subscription status")]
pub struct UnknownStatus;

/// A UTC calendar month, the billing period, written `YYYY-MM`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Month {
	year: i32,
	month: u8,
}

impl Month {
	/// The current month.
	pub fn current() -> Self {
		let seconds = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map_or(0, |elapsed| elapsed.as_secs());
		Self::from_unix_days((seconds / SECONDS_PER_DAY) as i64)
	}

	/// The month containing day `days` after 1970-01-01, by Howard
	/// Hinnant's `civil_from_days`.
	pub fn from_unix_days(days: i64) -> Self {
		let shifted = days + DAYS_FROM_YEAR_ZERO;
		let era = shifted.div_euclid(DAYS_PER_ERA);
		let day_of_era = shifted.rem_euclid(DAYS_PER_ERA);
		let year_of_era =
			(day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
		let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
		let shifted_month = (5 * day_of_year + 2) / 153;
		let month = if shifted_month < 10 {
			shifted_month + 3
		} else {
			shifted_month - 9
		};
		let year = year_of_era + era * 400 + i64::from(month <= 2);

		Self {
			year: year as i32,
			month: month as u8,
		}
	}

	/// The month after this one.
	#[must_use]
	pub fn next(self) -> Self {
		if self.month == MONTHS_PER_YEAR {
			Self {
				year: self.year + 1,
				month: 1,
			}
		} else {
			Self {
				month: self.month + 1,
				..self
			}
		}
	}

	/// 00:00 UTC on the first day, in Unix seconds, by Howard Hinnant's
	/// `days_from_civil`.
	pub fn start_unix_seconds(self) -> i64 {
		// Years start in March here, so a leap day ends its year.
		let year = i64::from(self.year) - i64::from(self.month <= 2);
		let era = year.div_euclid(YEARS_PER_ERA);
		let year_of_era = year - era * YEARS_PER_ERA;
		let months_since_march = (i64::from(self.month) + 9) % 12;
		// Days before each month from March, whose lengths run 31, 30, 31,
		// 30, 31 and repeat.
		let day_of_year = (153 * months_since_march + 2) / 5;
		let leap_days = year_of_era / 4 - year_of_era / 100;
		let day_of_era = year_of_era * DAYS_PER_YEAR + leap_days + day_of_year;
		let days = era * DAYS_PER_ERA + day_of_era - DAYS_FROM_YEAR_ZERO;

		days * SECONDS_PER_DAY as i64
	}

	/// The last second of the month, in Unix seconds.
	pub fn end_unix_seconds(self) -> i64 {
		self.next().start_unix_seconds() - 1
	}
}

const SECONDS_PER_DAY: u64 = 86_400;
const MONTHS_PER_YEAR: u8 = 12;
const DAYS_PER_YEAR: i64 = 365;
/// Years in a Gregorian cycle, after which the calendar repeats.
const YEARS_PER_ERA: i64 = 400;
/// Days from 0000-03-01 to 1970-01-01 in the proleptic Gregorian calendar.
const DAYS_FROM_YEAR_ZERO: i64 = 719_468;
/// Days in a 400-year Gregorian cycle.
const DAYS_PER_ERA: i64 = 146_097;

impl fmt::Display for Month {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(formatter, "{:04}-{:02}", self.year, self.month)
	}
}

impl FromStr for Month {
	type Err = InvalidMonth;

	fn from_str(text: &str) -> Result<Self, Self::Err> {
		let (year, month) = text.split_once('-').ok_or(InvalidMonth)?;
		let year = year.parse().map_err(|_| InvalidMonth)?;
		let month: u8 = month.parse().map_err(|_| InvalidMonth)?;

		if text.len() != "YYYY-MM".len() || !(1..=12).contains(&month) {
			return Err(InvalidMonth);
		}

		Ok(Self { year, month })
	}
}

/// A month that is not `YYYY-MM`.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("months are written YYYY-MM")]
pub struct InvalidMonth;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn months_come_from_unix_days() {
		assert_eq!(Month::from_unix_days(0).to_string(), "1970-01");
		// 2026-10-19 and the last day of a leap February.
		assert_eq!(Month::from_unix_days(20_745).to_string(), "2026-10");
		assert_eq!(Month::from_unix_days(19_782).to_string(), "2024-02");
		assert_eq!(Month::from_unix_days(19_783).to_string(), "2024-03");
		assert_eq!(Month::from_unix_days(-1).to_string(), "1969-12");
	}

	/// 2026-11-01T00:00:00Z.
	const NOVEMBER_2026: i64 = 1_793_491_200;
	/// 2024-03-01T00:00:00Z, the day after a leap day.
	const MARCH_2024: i64 = 1_709_251_200;
	/// Days around and far from the epoch, either side of it.
	const SAMPLE_DAYS: [i64; 6] = [-400_000, -1, 0, 19_782, 20_745, 400_000];

	#[test]
	fn months_know_their_bounds() {
		let month = |text: &str| text.parse::<Month>().expect("valid");
		assert_eq!(month("1970-01").start_unix_seconds(), 0);
		assert_eq!(month("2026-11").start_unix_seconds(), NOVEMBER_2026);
		assert_eq!(month("2024-02").end_unix_seconds(), MARCH_2024 - 1);
		assert_eq!(month("2026-12").next(), month("2027-01"));
		let day = SECONDS_PER_DAY as i64;

		for days in SAMPLE_DAYS {
			let start = Month::from_unix_days(days).start_unix_seconds();
			assert_eq!(
				Month::from_unix_days(start.div_euclid(day)),
				Month::from_unix_days(days)
			);
			assert!(start <= days * day);
		}
	}

	#[test]
	fn subscription_statuses_round_trip() {
		for status in SubscriptionStatus::ALL {
			assert_eq!(status.as_str().parse(), Ok(status));
		}

		assert!(SubscriptionStatus::PastDue.grants_plan());
		assert!(!SubscriptionStatus::Unpaid.grants_plan());
		assert!("bogus".parse::<SubscriptionStatus>().is_err());
	}

	#[test]
	fn only_paid_plans_meter_usage() {
		let customer = Some("cus_1".to_owned());
		let hobby = Terms {
			stripe_customer: customer.clone(),
			..Terms::of(Plan::Hobby)
		};
		let builder = Terms {
			stripe_customer: customer,
			..Terms::of(Plan::Builder)
		};
		assert_eq!(hobby.metered_customer(), None);
		assert_eq!(builder.metered_customer(), Some("cus_1"));
	}

	#[test]
	fn months_parse_and_print() {
		assert_eq!(
			"2026-10".parse::<Month>().map(|month| month.to_string()),
			Ok("2026-10".to_owned())
		);
		assert!("2026-13".parse::<Month>().is_err());
		assert!("2026-1".parse::<Month>().is_err());
		assert!("october".parse::<Month>().is_err());
	}

	#[test]
	fn only_the_free_plan_caps() {
		assert!(Terms::of(Plan::Hobby).is_full(1_000));
		assert!(!Terms::of(Plan::Hobby).is_full(999));
		assert!(!Terms::of(Plan::Builder).is_full(1_000_000));
		assert!(!Terms::of(Plan::Enterprise).is_full(u64::MAX));
	}
}
