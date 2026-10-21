//! Why an editor call failed.

use canvist_headless::HeadlessError;
use canvist_layout::LayoutError;
use canvist_paint::CpuError;

/// An error from the desktop editor.
#[derive(Debug, thiserror::Error)]
pub enum DesktopError {
	/// The engine rejected a call: a parse error, invalid arguments, or a
	/// command that failed.
	#[error(transparent)]
	Headless(#[from] HeadlessError),
	/// Layout failed, such as a measurer breaking its contract.
	#[error(transparent)]
	Layout(#[from] LayoutError),
	/// A document from another engine was handed to an editor: its schema is
	/// not the editor's.
	#[error("the document belongs to another engine; create it with the editor's engine")]
	ForeignDocument,
	/// Rendering to pixels failed.
	#[error(transparent)]
	Render(#[from] CpuError),
	/// The window host could not set up the GPU.
	#[cfg(feature = "winit")]
	#[error("the GPU could not be set up: {0}")]
	Gpu(String),
	/// The window host could not create its window.
	#[cfg(feature = "winit")]
	#[error("the window could not be created: {0}")]
	Window(#[from] winit::error::OsError),
	/// The winit event loop failed to start or stopped with an error.
	#[cfg(feature = "winit")]
	#[error("the event loop failed: {0}")]
	EventLoop(#[from] winit::error::EventLoopError),
}

/// A result with a [`DesktopError`].
pub type DesktopResult<T> = Result<T, DesktopError>;
