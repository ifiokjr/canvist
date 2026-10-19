//! Identifiers: projects, environments, documents, and random secrets.

use std::fmt;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

/// The longest document name, in bytes.
pub const MAX_DOCUMENT_NAME: usize = 256;
/// The longest environment name, in bytes.
pub const MAX_ENVIRONMENT_NAME: usize = 64;

/// A project id such as `prj_k3f9a2m1q8x7`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(Arc<str>);

impl ProjectId {
	/// A new random project id.
	pub fn generate() -> Self {
		Self(format!("prj_{}", random_token(10)).into())
	}

	/// Wrap a stored id.
	pub fn new(id: impl Into<Arc<str>>) -> Self {
		Self(id.into())
	}

	/// The id as a string.
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl fmt::Display for ProjectId {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.0)
	}
}

/// A name that failed validation.
#[derive(Debug, thiserror::Error)]
#[error("invalid {kind}: {reason}")]
pub struct NameError {
	kind: &'static str,
	reason: &'static str,
}

/// An environment name such as `production` or `staging`: lowercase ASCII
/// letters, digits, and dashes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Environment(Arc<str>);

impl Environment {
	/// Validate `name`.
	pub fn parse(name: &str) -> Result<Self, NameError> {
		let error = |reason| {
			NameError {
				kind: "environment",
				reason,
			}
		};

		if name.is_empty() || name.len() > MAX_ENVIRONMENT_NAME {
			return Err(error("must be 1 to 64 bytes"));
		}

		let valid = name
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');

		if !valid {
			return Err(error("use lowercase letters, digits, and dashes"));
		}

		Ok(Self(name.into()))
	}

	/// The name.
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl TryFrom<String> for Environment {
	type Error = NameError;

	fn try_from(name: String) -> Result<Self, Self::Error> {
		Self::parse(&name)
	}
}

impl From<Environment> for String {
	fn from(environment: Environment) -> Self {
		environment.0.to_string()
	}
}

impl fmt::Display for Environment {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.0)
	}
}

/// A document name chosen by the customer: 1 to 256 bytes of ASCII letters,
/// digits, and `-_.~:@/`. Slashes let names nest, as in `team/notes`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DocumentName(Arc<str>);

impl DocumentName {
	/// Validate `name`.
	pub fn parse(name: &str) -> Result<Self, NameError> {
		let error = |reason| {
			NameError {
				kind: "document name",
				reason,
			}
		};

		if name.is_empty() || name.len() > MAX_DOCUMENT_NAME {
			return Err(error("must be 1 to 256 bytes"));
		}

		if !name.bytes().all(is_name_byte) {
			return Err(error("use ASCII letters, digits, and -_.~:@/"));
		}

		Ok(Self(name.into()))
	}

	/// The name.
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

fn is_name_byte(byte: u8) -> bool {
	byte.is_ascii_alphanumeric() || b"-_.~:@/".contains(&byte)
}

impl TryFrom<String> for DocumentName {
	type Error = NameError;

	fn try_from(name: String) -> Result<Self, Self::Error> {
		Self::parse(&name)
	}
}

impl From<DocumentName> for String {
	fn from(name: DocumentName) -> Self {
		name.0.to_string()
	}
}

impl fmt::Display for DocumentName {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.0)
	}
}

/// Where a document lives: one project's environment.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentKey {
	/// The owning project.
	pub project: ProjectId,
	/// The project's environment.
	pub environment: Environment,
	/// The customer's name for the document.
	pub name: DocumentName,
}

impl fmt::Display for DocumentKey {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			formatter,
			"{}/{}/{}",
			self.project, self.environment, self.name
		)
	}
}

/// `length` random characters from a 32-letter, lowercase alphabet that
/// avoids look-alikes: 5 bits of entropy each.
pub fn random_token(length: usize) -> String {
	const ALPHABET: &[u8; 32] = b"abcdefghjkmnpqrstuvwxyz023456789";
	const FIVE_BITS: u8 = 0b1_1111;
	let mut bytes = vec![0_u8; length];
	getrandom::fill(&mut bytes).expect("the operating system provides randomness");
	bytes
		.into_iter()
		.map(|byte| char::from(ALPHABET[usize::from(byte & FIVE_BITS)]))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn document_names_allow_nesting_but_not_spaces() {
		assert!(DocumentName::parse("team/notes-2026.md").is_ok());
		assert!(DocumentName::parse("").is_err());
		assert!(DocumentName::parse("has space").is_err());
		assert!(DocumentName::parse(&"a".repeat(257)).is_err());
	}

	#[test]
	fn environments_are_lowercase() {
		assert!(Environment::parse("production").is_ok());
		assert!(Environment::parse("Staging").is_err());
	}

	#[test]
	fn random_tokens_have_the_requested_length() {
		let token = random_token(32);
		assert_eq!(token.len(), 32);
		assert_ne!(token, random_token(32));
	}
}
