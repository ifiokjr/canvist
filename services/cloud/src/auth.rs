//! Who is calling: API keys for servers, short-lived tokens for clients.
//!
//! - An **API key** (`ck_…`) belongs to one project environment and is a
//!   server-side secret. It authorizes every REST call and can open any
//!   document. The store keeps only its SHA-256 hash.
//! - A **client token** is an HS256 JWT that a customer's server mints with
//!   `POST /v1/tokens`. It names one document, or a prefix ending in `*`, a
//!   `read` or `write` scope, and expires within a day. Browsers pass it as
//!   the `token` query parameter (y-websocket) or in the auth message
//!   (Hocuspocus).

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::Hmac;
use hmac::Mac;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::names::DocumentName;
use crate::names::Environment;
use crate::names::ProjectId;
use crate::names::random_token;

/// The longest a client token may live, in seconds.
pub const MAX_TOKEN_SECONDS: u64 = 24 * 60 * 60;
/// Random characters in an API key: 200 bits.
const API_KEY_LENGTH: usize = 40;
/// Clock skew tolerated when checking `exp`, in seconds.
const LEEWAY_SECONDS: u64 = 5;

/// What a session may do with a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
	/// Receive the document and presence; updates are refused.
	Read,
	/// Read and edit.
	Write,
}

/// The claims inside a client token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
	/// The project.
	pub prj: ProjectId,
	/// The project environment.
	pub env: Environment,
	/// The customer's id for the user, echoed in webhooks and logs.
	pub sub: String,
	/// A document name, or a prefix ending in `*`.
	pub doc: String,
	/// What the holder may do.
	pub scope: Scope,
	/// Issued at, in Unix seconds.
	pub iat: u64,
	/// Expires at, in Unix seconds.
	pub exp: u64,
}

impl Claims {
	/// Whether these claims cover the document `name`.
	pub fn covers(&self, name: &DocumentName) -> bool {
		match self.doc.strip_suffix('*') {
			Some(prefix) => name.as_str().starts_with(prefix),
			None => self.doc == name.as_str(),
		}
	}
}

/// Who a request or session acts for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
	/// The project.
	pub project: ProjectId,
	/// The project environment.
	pub environment: Environment,
	/// How they authenticated.
	pub access: Access,
}

/// How a principal authenticated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Access {
	/// A server holding an API key: full access to the environment.
	Key,
	/// A client token, limited by its claims.
	Token(Claims),
}

impl Principal {
	/// The scope this principal has on document `name`, if any.
	pub fn scope_for(&self, name: &DocumentName) -> Option<Scope> {
		match &self.access {
			Access::Key => Some(Scope::Write),
			Access::Token(claims) => claims.covers(name).then_some(claims.scope),
		}
	}

	/// The user id for logs and webhooks: the token's subject, or `server`.
	pub fn subject(&self) -> &str {
		match &self.access {
			Access::Key => "server",
			Access::Token(claims) => &claims.sub,
		}
	}
}

/// Why a credential was refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
	/// No credential was sent.
	#[error("missing credentials")]
	Missing,
	/// The credential is malformed, forged, or unknown.
	#[error("invalid credentials")]
	Invalid,
	/// The token expired.
	#[error("token expired")]
	Expired,
	/// The credential is valid but does not cover this document.
	#[error("this credential cannot open this document")]
	Forbidden,
}

/// A newly created API key: shown once, then only its hash is kept.
#[derive(Debug)]
pub struct NewApiKey {
	/// The full key, to hand to the customer.
	pub secret: String,
	/// The hash the store keeps.
	pub hash: KeyHash,
	/// The first characters, safe to display.
	pub prefix: String,
}

/// The SHA-256 hash of an API key.
pub type KeyHash = [u8; 32];

/// Create a random API key.
pub fn generate_api_key() -> NewApiKey {
	let secret = format!("ck_{}", random_token(API_KEY_LENGTH));
	NewApiKey {
		hash: hash_api_key(&secret),
		prefix: secret[..10].to_owned(),
		secret,
	}
}

/// The stored hash of `key`.
pub fn hash_api_key(key: &str) -> KeyHash {
	Sha256::digest(key.as_bytes()).into()
}

/// Whether `credential` looks like an API key rather than a token.
pub fn is_api_key(credential: &str) -> bool {
	credential.starts_with("ck_")
}

/// Signs and verifies client tokens.
#[derive(Clone)]
pub struct TokenSigner {
	key: Vec<u8>,
}

impl std::fmt::Debug for TokenSigner {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("TokenSigner(..)")
	}
}

const HEADER: &str = r#"{"alg":"HS256","typ":"JWT"}"#;

impl TokenSigner {
	/// A signer keyed by `secret`.
	pub fn new(secret: &str) -> Self {
		Self {
			key: secret.as_bytes().to_vec(),
		}
	}

	/// Sign `claims` as a compact JWT.
	pub fn sign(&self, claims: &Claims) -> String {
		let header = URL_SAFE_NO_PAD.encode(HEADER);
		let payload =
			URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("claims always serialize"));
		let signing_input = format!("{header}.{payload}");
		let signature = URL_SAFE_NO_PAD.encode(self.mac(&signing_input).finalize().into_bytes());
		format!("{signing_input}.{signature}")
	}

	/// Verify `token` at `now` (Unix seconds) and return its claims.
	pub fn verify(&self, token: &str, now: u64) -> Result<Claims, AuthError> {
		let (signing_input, signature) = token.rsplit_once('.').ok_or(AuthError::Invalid)?;
		let (header, payload) = signing_input.split_once('.').ok_or(AuthError::Invalid)?;
		let signature = URL_SAFE_NO_PAD
			.decode(signature)
			.map_err(|_| AuthError::Invalid)?;

		self.mac(signing_input)
			.verify_slice(&signature)
			.map_err(|_| AuthError::Invalid)?;

		if !is_hs256(header) {
			return Err(AuthError::Invalid);
		}

		let payload = URL_SAFE_NO_PAD
			.decode(payload)
			.map_err(|_| AuthError::Invalid)?;
		let claims: Claims = serde_json::from_slice(&payload).map_err(|_| AuthError::Invalid)?;

		if claims.exp + LEEWAY_SECONDS <= now {
			return Err(AuthError::Expired);
		}

		Ok(claims)
	}

	fn mac(&self, input: &str) -> Hmac<Sha256> {
		let mut mac =
			Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC accepts keys of any length");
		mac.update(input.as_bytes());
		mac
	}
}

/// Whether a JWT header names HS256. The signature is checked first, so
/// only headers we signed reach this, but `alg` is still pinned so a key
/// can never be read as some other algorithm's.
fn is_hs256(header: &str) -> bool {
	#[derive(Deserialize)]
	struct Header {
		alg: String,
	}

	URL_SAFE_NO_PAD
		.decode(header)
		.ok()
		.and_then(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
		.is_some_and(|header| header.alg == "HS256")
}

/// The current time in Unix seconds.
pub fn unix_now() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_secs())
}

/// The bearer credential in an `Authorization` header value.
pub fn bearer(header: &str) -> Option<&str> {
	let (scheme, credential) = header.split_once(' ')?;
	scheme
		.eq_ignore_ascii_case("bearer")
		.then(|| credential.trim())
		.filter(|credential| !credential.is_empty())
}

/// Constant-time comparison of two secrets.
pub fn secrets_match(given: &str, expected: &str) -> bool {
	let given = Sha256::digest(given.as_bytes());
	let expected = Sha256::digest(expected.as_bytes());
	given
		.iter()
		.zip(expected.iter())
		.fold(0_u8, |difference, (left, right)| {
			difference | (left ^ right)
		})
		== 0
}

#[cfg(test)]
mod tests {
	use super::*;

	fn claims(doc: &str, exp: u64) -> Claims {
		Claims {
			prj: ProjectId::new("prj_test"),
			env: Environment::parse("production").expect("valid"),
			sub: "user-1".to_owned(),
			doc: doc.to_owned(),
			scope: Scope::Write,
			iat: 100,
			exp,
		}
	}

	fn name(name: &str) -> DocumentName {
		DocumentName::parse(name).expect("valid")
	}

	#[test]
	fn tokens_round_trip() {
		let signer = TokenSigner::new("0123456789abcdef0123456789abcdef");
		let token = signer.sign(&claims("notes", 200));
		assert_eq!(signer.verify(&token, 150), Ok(claims("notes", 200)));
	}

	#[test]
	fn expired_tokens_are_refused() {
		let signer = TokenSigner::new("0123456789abcdef0123456789abcdef");
		let token = signer.sign(&claims("notes", 200));
		assert_eq!(signer.verify(&token, 300), Err(AuthError::Expired));
	}

	#[test]
	fn forged_and_tampered_tokens_are_refused() {
		let signer = TokenSigner::new("0123456789abcdef0123456789abcdef");
		let other = TokenSigner::new("another secret that is long enough!");
		let token = other.sign(&claims("notes", 200));
		assert_eq!(signer.verify(&token, 150), Err(AuthError::Invalid));

		let genuine = signer.sign(&claims("notes", 200));
		let (input, signature) = genuine.rsplit_once('.').expect("three parts");
		let (header, _) = input.split_once('.').expect("three parts");
		let forged_payload =
			URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims("*", 200)).expect("serializes"));
		let tampered = format!("{header}.{forged_payload}.{signature}");
		assert_eq!(signer.verify(&tampered, 150), Err(AuthError::Invalid));
		assert_eq!(signer.verify("not a token", 150), Err(AuthError::Invalid));
	}

	#[test]
	fn the_none_algorithm_is_refused() {
		let signer = TokenSigner::new("0123456789abcdef0123456789abcdef");
		let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
		let payload =
			URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims("notes", 200)).expect("serializes"));
		assert_eq!(
			signer.verify(&format!("{header}.{payload}."), 150),
			Err(AuthError::Invalid)
		);
	}

	#[test]
	fn prefixes_cover_nested_documents() {
		let prefix = claims("team/*", 200);
		assert!(prefix.covers(&name("team/notes")));
		assert!(!prefix.covers(&name("other/notes")));
		assert!(claims("notes", 200).covers(&name("notes")));
		assert!(!claims("notes", 200).covers(&name("notes2")));
	}

	#[test]
	fn api_keys_hash_deterministically() {
		let key = generate_api_key();
		assert!(is_api_key(&key.secret));
		assert_eq!(hash_api_key(&key.secret), key.hash);
		assert!(key.secret.starts_with(&key.prefix));
	}

	#[test]
	fn bearer_headers_parse() {
		assert_eq!(bearer("Bearer ck_abc"), Some("ck_abc"));
		assert_eq!(bearer("bearer  tok "), Some("tok"));
		assert_eq!(bearer("Basic abc"), None);
		assert_eq!(bearer("Bearer "), None);
	}

	#[test]
	fn secrets_compare_by_value() {
		assert!(secrets_match("abc", "abc"));
		assert!(!secrets_match("abc", "abd"));
	}
}
