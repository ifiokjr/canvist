//! DOM interaction utilities.
//!
//! Helpers for working with the browser DOM — event listeners, accessibility
//! shadow DOM, and input handling.

use wasm_bindgen::prelude::*;
use web_sys::Document;
use web_sys::Element;

/// Create a hidden accessibility container element.
///
/// This creates an off-screen `<div>` that mirrors the document structure
/// as plain HTML elements, allowing screen readers to interpret the content.
/// This is the same technique used by Google Docs.
///
/// # Errors
///
/// Returns an error if DOM manipulation fails.
#[allow(dead_code)]
pub fn create_accessibility_container(document: &Document) -> Result<Element, JsValue> {
	let container = document.create_element("div")?;
	container.set_attribute("role", "textbox")?;
	container.set_attribute("aria-multiline", "true")?;
	container.set_attribute("aria-label", "Document editor")?;

	// Position off-screen but still accessible to screen readers.
	container.set_attribute(
		"style",
		"position: absolute; left: -9999px; top: -9999px; width: 1px; height: 1px; overflow: hidden;",
	)?;

	Ok(container)
}

/// Log a message to the browser console.
#[allow(dead_code)]
pub fn console_log(msg: &str) {
	web_sys::console::log_1(&JsValue::from_str(msg));
}
