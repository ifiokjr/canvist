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
	/// The Stripe customer to report usage for.
	#[serde(skip)]
	pub stripe_customer: Option<String>,
}

impl Terms {
	/// The default terms of `plan`.
	pub fn of(plan: Plan) -> Self {
		Self {
			plan,
			included_documents: plan.included_documents(),
			stripe_customer: None,
		}
	}

	/// Whether `active` documents leave no room for another under a cap.
	pub fn is_full(&self, active: u64) -> bool {
		self.plan.is_capped()
			&& self
				.included_documents
				.is_some_and(|included| active >= included)
	}
}

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
}

const SECONDS_PER_DAY: u64 = 86_400;
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
