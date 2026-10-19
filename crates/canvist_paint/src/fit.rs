//! Where an image paint's pixels go inside its node's box.

use canvist_scene::ImageFit;
use kurbo::Affine;
use kurbo::Size;

/// The transform from an image's pixel space (`(0, 0)` to `image`) to a
/// node box of `size`, for `fit`. Tiling draws the image once at its natural
/// size; renderers repeat it with the brush's extend mode.
#[must_use]
pub fn image_transform(fit: ImageFit, image: Size, size: Size) -> Affine {
	if image.width <= 0.0 || image.height <= 0.0 {
		return Affine::IDENTITY;
	}

	let sx = size.width / image.width;
	let sy = size.height / image.height;

	let scale = match fit {
		ImageFit::Fill => sx.max(sy),
		ImageFit::Fit => sx.min(sy),
		ImageFit::Stretch => return Affine::scale_non_uniform(sx, sy),
		ImageFit::Tile => return Affine::IDENTITY,
	};

	let offset = (
		(size.width - image.width * scale) / 2.0,
		(size.height - image.height * scale) / 2.0,
	);

	Affine::translate(offset) * Affine::scale(scale)
}

#[cfg(test)]
mod tests {
	use kurbo::Point;

	use super::*;

	/// A 2:1 image in a square box.
	const WIDE: Size = Size::new(200.0, 100.0);

	#[test]
	fn fill_covers_and_fit_contains() {
		let image = WIDE;
		let size = Size::new(100.0, 100.0);
		let fill = image_transform(ImageFit::Fill, image, size);
		let fit = image_transform(ImageFit::Fit, image, size);

		assert_eq!(fill * Point::new(0.0, 0.0), Point::new(-50.0, 0.0));
		assert_eq!(
			fill * Point::new(WIDE.width, WIDE.height),
			Point::new(150.0, 100.0)
		);
		assert_eq!(fit * Point::new(0.0, 0.0), Point::new(0.0, 25.0));
		assert_eq!(
			fit * Point::new(WIDE.width, WIDE.height),
			Point::new(100.0, 75.0)
		);
	}
}
