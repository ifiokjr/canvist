//! Decoded images, by the asset ids scenes name them with.

use std::sync::Arc;

use peniko::Blob;
use peniko::ImageAlphaType;
use peniko::ImageData;
use peniko::ImageFormat;
use rustc_hash::FxHashMap;

/// Decoded images by asset id. Scenes store asset ids (for example a
/// content hash); the host loads the bytes and registers the pixels here.
#[derive(Clone, Debug, Default)]
pub struct Assets {
	images: FxHashMap<Arc<str>, ImageData>,
}

impl Assets {
	/// No images.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Register straight-alpha RGBA8 pixels, `width` by `height`, as `asset`.
	///
	/// # Panics
	///
	/// Panics when `pixels` is not `width * height * 4` bytes long.
	pub fn insert_rgba8(&mut self, asset: &str, width: u32, height: u32, pixels: Vec<u8>) {
		assert_eq!(
			pixels.len(),
			width as usize * height as usize * 4,
			"RGBA8 needs four bytes per pixel"
		);
		let image = ImageData {
			data: Blob::new(Arc::new(pixels)),
			format: ImageFormat::Rgba8,
			alpha_type: ImageAlphaType::Alpha,
			width,
			height,
		};
		self.images.insert(asset.into(), image);
	}

	/// The image registered as `asset`.
	#[must_use]
	pub fn get(&self, asset: &str) -> Option<&ImageData> {
		self.images.get(asset)
	}

	/// Forget `asset`.
	pub fn remove(&mut self, asset: &str) {
		self.images.remove(asset);
	}
}
