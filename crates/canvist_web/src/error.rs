//! Errors as JavaScript `Error`s named `CanvistError`, with the same stable
//! `code` strings as `@canvist/headless`.

use canvist_headless::ErrorCode;
use canvist_headless::HeadlessError;
use canvist_layout::LayoutError;
use wasm_bindgen::JsValue;

pub(crate) type JsResult<T> = Result<T, JsValue>;

fn canvist_error(message: &str, code: ErrorCode) -> JsValue {
	let error = js_sys::Error::new(message);
	error.set_name("CanvistError");
	// Setting a property on a fresh `Error` object cannot fail.
	let _ = js_sys::Reflect::set(&error, &"code".into(), &code.as_str().into());
	error.into()
}

#[allow(
	clippy::needless_pass_by_value,
	reason = "a `map_err` callback receives the error by value"
)]
pub(crate) fn headless(error: HeadlessError) -> JsValue {
	canvist_error(&error.to_string(), error.code())
}

pub(crate) fn render(error: &LayoutError) -> JsValue {
	canvist_error(&error.to_string(), ErrorCode::Render)
}

pub(crate) fn invalid_options(reason: &str) -> JsValue {
	canvist_error(
		&format!("invalid editor options: {reason}"),
		ErrorCode::Config,
	)
}

pub(crate) fn invalid_args(reason: &str) -> JsValue {
	canvist_error(
		&format!("invalid arguments: {reason}"),
		ErrorCode::InvalidArgs,
	)
}
