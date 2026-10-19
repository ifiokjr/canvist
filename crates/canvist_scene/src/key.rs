//! Fractional keys that order siblings.
//!
//! A key is a base-62 string made of an integer part and a fraction, as in
//! the `fractional-indexing` package used by Figma-style editors: the first
//! character gives the integer part's length (`a`–`z` for positive lengths
//! 2–27, `A`–`Z` for negative ones), and keys compare as plain ASCII strings.
//! A key always exists strictly between any two keys, so inserting or moving
//! a node creates one new key and rewrites no sibling. Appending keeps keys
//! short: appending n nodes grows keys by `O(log n)` characters.

use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const ZERO: u8 = b'0';
const LAST: u8 = b'z';
/// The smallest integer part, which nothing can precede.
const SMALLEST_INTEGER: &str = "A00000000000000000000000000";

/// A fractional key: an immutable, cheaply cloned, validated string.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FracKey(Arc<str>);

/// Why a string is not a valid [`FracKey`], or no key could be generated.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
	/// The string is not a well-formed key.
	#[error("invalid fractional key `{0}`")]
	Invalid(String),
	/// `between` was asked for a key between two keys out of order.
	#[error("fractional key `{0}` is not before `{1}`")]
	OutOfOrder(String, String),
	/// The integer part cannot grow or shrink any further.
	#[error("fractional keys exhausted")]
	Exhausted,
}

impl FracKey {
	/// Parse and validate a key.
	pub fn parse(key: &str) -> Result<Self, KeyError> {
		validate(key.as_bytes()).map_err(|()| KeyError::Invalid(key.to_owned()))?;
		Ok(Self(key.into()))
	}

	/// The first key of an empty sibling list: `a0`.
	#[must_use]
	pub fn first() -> Self {
		Self("a0".into())
	}

	/// The key as a string.
	#[must_use]
	pub fn as_str(&self) -> &str {
		&self.0
	}

	/// A key strictly between `before` and `after`, where `None` means the
	/// start or end of the list.
	pub fn between(before: Option<&Self>, after: Option<&Self>) -> Result<Self, KeyError> {
		let before = before.map(|key| key.0.as_bytes());
		let after = after.map(|key| key.0.as_bytes());
		let key = key_between(before, after)?;

		Ok(Self(String::from_utf8(key).unwrap_or_default().into()))
	}

	/// `count` increasing keys strictly between `before` and `after`, for
	/// inserting many siblings at once.
	pub fn n_between(
		before: Option<&Self>,
		after: Option<&Self>,
		count: usize,
	) -> Result<Vec<Self>, KeyError> {
		let mut keys = Vec::with_capacity(count);
		let mut previous = before.cloned();

		for _ in 0..count {
			let key = Self::between(previous.as_ref(), after)?;
			previous = Some(key.clone());
			keys.push(key);
		}

		Ok(keys)
	}
}

impl fmt::Debug for FracKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "FracKey({:?})", &*self.0)
	}
}

impl fmt::Display for FracKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.0)
	}
}

impl PartialOrd for FracKey {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for FracKey {
	fn cmp(&self, other: &Self) -> Ordering {
		self.0.as_bytes().cmp(other.0.as_bytes())
	}
}

fn digit(byte: u8) -> Option<usize> {
	DIGITS.iter().position(|&candidate| candidate == byte)
}

fn integer_length(head: u8) -> Option<usize> {
	match head {
		b'a'..=b'z' => Some(usize::from(head - b'a') + 2),
		b'A'..=b'Z' => Some(usize::from(b'Z' - head) + 2),
		_ => None,
	}
}

fn split(key: &[u8]) -> Result<(&[u8], &[u8]), ()> {
	let length = integer_length(*key.first().ok_or(())?).ok_or(())?;

	if length > key.len() {
		return Err(());
	}

	Ok(key.split_at(length))
}

fn validate(key: &[u8]) -> Result<(), ()> {
	let (integer, fraction) = split(key)?;

	if key == SMALLEST_INTEGER.as_bytes() || fraction.last() == Some(&ZERO) {
		return Err(());
	}

	if integer[1..]
		.iter()
		.chain(fraction)
		.all(|&byte| digit(byte).is_some())
	{
		Ok(())
	} else {
		Err(())
	}
}

fn key_between(before: Option<&[u8]>, after: Option<&[u8]>) -> Result<Vec<u8>, KeyError> {
	if let (Some(before), Some(after)) = (before, after)
		&& before >= after
	{
		return Err(KeyError::OutOfOrder(text(before), text(after)));
	}

	match (before, after) {
		(None, None) => Ok(b"a0".to_vec()),
		(None, Some(after)) => key_before(after),
		(Some(before), None) => key_after(before),
		(Some(before), Some(after)) => key_inside(before, after),
	}
}

fn key_before(after: &[u8]) -> Result<Vec<u8>, KeyError> {
	let (integer, fraction) = split(after).map_err(|()| KeyError::Invalid(text(after)))?;

	if integer == SMALLEST_INTEGER.as_bytes() {
		return Ok([integer, &midpoint(&[], Some(fraction))].concat());
	}

	if integer.len() < after.len() {
		return Ok(integer.to_vec());
	}

	decrement(integer).ok_or(KeyError::Exhausted)
}

fn key_after(before: &[u8]) -> Result<Vec<u8>, KeyError> {
	let (integer, fraction) = split(before).map_err(|()| KeyError::Invalid(text(before)))?;

	Ok(increment(integer).unwrap_or_else(|| [integer, &midpoint(fraction, None)].concat()))
}

fn key_inside(before: &[u8], after: &[u8]) -> Result<Vec<u8>, KeyError> {
	let (before_integer, before_fraction) =
		split(before).map_err(|()| KeyError::Invalid(text(before)))?;
	let (after_integer, after_fraction) =
		split(after).map_err(|()| KeyError::Invalid(text(after)))?;

	if before_integer == after_integer {
		return Ok([
			before_integer,
			&midpoint(before_fraction, Some(after_fraction)),
		]
		.concat());
	}

	let next = increment(before_integer).ok_or(KeyError::Exhausted)?;

	if next.as_slice() < after {
		return Ok(next);
	}

	Ok([before_integer, &midpoint(before_fraction, None)].concat())
}

/// A fraction strictly between `low` and `high` (`None` meaning 1), neither
/// of which ends in the zero digit.
fn midpoint(low: &[u8], high: Option<&[u8]>) -> Vec<u8> {
	if let Some(high) = high {
		let shared = high
			.iter()
			.enumerate()
			.take_while(|&(index, &byte)| low.get(index).copied().unwrap_or(ZERO) == byte)
			.count();

		if shared > 0 {
			let tail = midpoint(low.get(shared..).unwrap_or(&[]), Some(&high[shared..]));
			return [&high[..shared], &tail].concat();
		}
	}

	let low_digit = low.first().and_then(|&byte| digit(byte)).unwrap_or(0);
	let high_digit = high
		.and_then(|high| high.first())
		.and_then(|&byte| digit(byte))
		.unwrap_or(DIGITS.len());

	if high_digit - low_digit > 1 {
		return vec![DIGITS[(low_digit + high_digit).div_ceil(2)]];
	}

	if let Some(high) = high
		&& high.len() > 1
	{
		return vec![high[0]];
	}

	let tail = midpoint(low.get(1..).unwrap_or(&[]), None);
	[&[DIGITS[low_digit]], tail.as_slice()].concat()
}

/// Add one to an integer part: carry through the digits, and when every
/// digit overflows, grow to the next head letter (one digit longer for
/// positive integers, one shorter for negative ones).
fn increment(integer: &[u8]) -> Option<Vec<u8>> {
	let (&head, digits) = integer.split_first()?;
	let mut digits = digits.to_vec();

	if step_digits(&mut digits, 1)? {
		return Some(join(head, &digits));
	}

	match head {
		b'Z' => Some(vec![b'a', ZERO]),
		b'z' => None,
		_ if head + 1 > b'a' => Some(join(head + 1, &[digits, vec![ZERO]].concat())),
		_ => Some(join(head + 1, &digits[..digits.len().saturating_sub(1)])),
	}
}

/// Subtract one from an integer part: the mirror image of [`increment`].
fn decrement(integer: &[u8]) -> Option<Vec<u8>> {
	let (&head, digits) = integer.split_first()?;
	let mut digits = digits.to_vec();

	if step_digits(&mut digits, -1)? {
		return Some(join(head, &digits));
	}

	match head {
		b'a' => Some(vec![b'Z', LAST]),
		b'A' => None,
		_ if head - 1 < b'Z' => Some(join(head - 1, &[digits, vec![LAST]].concat())),
		_ => Some(join(head - 1, &digits[..digits.len().saturating_sub(1)])),
	}
}

/// Add `delta` (1 or -1) to base-62 digits in place, carrying from the
/// right. Returns `Some(false)` when every digit wrapped around, and `None`
/// for a byte that is not a digit.
fn step_digits(digits: &mut [u8], delta: isize) -> Option<bool> {
	let (wrap_from, wrap_to) = if delta > 0 {
		(LAST, ZERO)
	} else {
		(ZERO, LAST)
	};

	for byte in digits.iter_mut().rev() {
		if *byte == wrap_from {
			*byte = wrap_to;
			continue;
		}

		*byte = DIGITS[digit(*byte)?.checked_add_signed(delta)?];
		return Some(true);
	}

	Some(false)
}

fn join(head: u8, digits: &[u8]) -> Vec<u8> {
	[&[head], digits].concat()
}

fn text(bytes: &[u8]) -> String {
	String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
	use proptest::prelude::*;

	use super::*;

	fn key(value: &str) -> FracKey {
		FracKey::parse(value).expect("valid key")
	}

	fn between(before: Option<&str>, after: Option<&str>) -> String {
		let before = before.map(key);
		let after = after.map(key);

		FracKey::between(before.as_ref(), after.as_ref())
			.expect("keys in order")
			.to_string()
	}

	#[test]
	fn matches_the_reference_implementation() {
		assert_eq!(between(None, None), "a0");
		assert_eq!(between(None, Some("a0")), "Zz");
		assert_eq!(between(Some("a0"), None), "a1");
		assert_eq!(between(Some("a0"), Some("a1")), "a0V");
		assert_eq!(between(Some("a0V"), Some("a1")), "a0l");
		assert_eq!(between(Some("Zz"), Some("a0")), "ZzV");
		assert_eq!(between(Some("Zz"), Some("a01")), "a0");
		assert_eq!(between(None, Some("a0V")), "a0");
		assert_eq!(between(Some("b125"), Some("b129")), "b127");
		assert_eq!(between(Some("a0"), Some("a0V")), "a0G");
		assert_eq!(between(Some("az"), None), "b00");
		assert_eq!(between(Some("Zy"), None), "Zz");
		assert_eq!(
			between(Some("zzzzzzzzzzzzzzzzzzzzzzzzzzz"), None),
			"zzzzzzzzzzzzzzzzzzzzzzzzzzzV"
		);
	}

	#[test]
	fn rejects_malformed_keys() {
		for bad in ["", "a", "a00", "A00000000000000000000000000", "b!", "0a"] {
			assert!(FracKey::parse(bad).is_err(), "{bad} should be invalid");
		}

		let low = key("a1");
		let high = key("a0");
		assert!(FracKey::between(Some(&low), Some(&high)).is_err());
	}

	#[test]
	fn appending_keeps_keys_short() {
		let keys = FracKey::n_between(None, None, 10_000).expect("room");

		assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
		assert!(keys.iter().all(|key| key.as_str().len() <= 4));
	}

	fn position(len: usize, front: bool, offset: usize) -> usize {
		if front {
			offset.min(len)
		} else {
			len.saturating_sub(offset)
		}
	}

	proptest! {
		#[test]
		fn a_key_fits_between_any_two(choices in prop::collection::vec((any::<bool>(), 0..8usize), 1..60)) {
			let mut keys = vec![FracKey::first()];

			for (front, offset) in choices {
				let index = position(keys.len(), front, offset);
				let before = index.checked_sub(1).map(|at| keys[at].clone());
				let after = keys.get(index).cloned();
				let key = FracKey::between(before.as_ref(), after.as_ref()).expect("room");

				prop_assert!(before.as_ref().is_none_or(|before| before < &key));
				prop_assert!(after.as_ref().is_none_or(|after| &key < after));
				prop_assert!(FracKey::parse(key.as_str()).is_ok());
				keys.insert(index, key);
			}
		}
	}
}
