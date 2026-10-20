//! Rendering a vello scene to a window with wgpu: Metal on macOS, DX12 or
//! Vulkan on Windows, and Vulkan or GL on Linux.

use std::sync::Arc;

use canvist_paint::peniko::Color;
use vello::AaConfig;
use vello::RenderParams;
use vello::Renderer;
use vello::RendererOptions;
use vello::Scene;
use vello::util::RenderContext;
use vello::util::RenderSurface;
use vello::wgpu;
use winit::window::Window;

use crate::DesktopError;
use crate::DesktopResult;

/// A window's GPU surface and the vello renderer that draws into it.
pub struct GpuSurface {
	context: RenderContext,
	surface: RenderSurface<'static>,
	renderer: Renderer,
}

impl std::fmt::Debug for GpuSurface {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("GpuSurface")
			.field("width", &self.surface.config.width)
			.field("height", &self.surface.config.height)
			.finish_non_exhaustive()
	}
}

impl GpuSurface {
	/// Set up the GPU for `window`, sized to its inner size. This blocks
	/// while wgpu picks an adapter and vello compiles its shaders.
	pub fn new(window: Arc<Window>) -> DesktopResult<Self> {
		let size = window.inner_size();
		let mut context = RenderContext::new();
		let surface = pollster::block_on(context.create_surface(
			window,
			size.width.max(1),
			size.height.max(1),
			wgpu::PresentMode::AutoVsync,
		))
		.map_err(gpu)?;
		let device = &context.devices[surface.dev_id].device;
		let renderer = Renderer::new(
			device,
			RendererOptions {
				antialiasing_support: vello::AaSupport::area_only(),
				..RendererOptions::default()
			},
		)
		.map_err(gpu)?;

		Ok(Self {
			context,
			surface,
			renderer,
		})
	}

	/// Resize to `width` by `height` physical pixels.
	pub fn resize(&mut self, width: u32, height: u32) {
		let (width, height) = (width.max(1), height.max(1));

		if (width, height) != (self.surface.config.width, self.surface.config.height) {
			self.context
				.resize_surface(&mut self.surface, width, height);
		}
	}

	/// The surface size in physical pixels.
	#[must_use]
	pub fn size(&self) -> (u32, u32) {
		(self.surface.config.width, self.surface.config.height)
	}

	/// Draw `scene` over `background` and present it. A frame the surface
	/// cannot take right now (the window is hidden, or was resized under
	/// it) is skipped, and the surface is reconfigured when it went stale.
	pub fn render(&mut self, scene: &Scene, background: Color) -> DesktopResult<()> {
		let handle = &self.context.devices[self.surface.dev_id];
		let (width, height) = self.size();

		self.renderer
			.render_to_texture(
				&handle.device,
				&handle.queue,
				scene,
				&self.surface.target_view,
				&RenderParams {
					base_color: background,
					width,
					height,
					antialiasing_method: AaConfig::Area,
				},
			)
			.map_err(gpu)?;

		let texture = match self.surface.surface.get_current_texture() {
			wgpu::CurrentSurfaceTexture::Success(texture)
			| wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
			wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
				self.context.configure_surface(&self.surface);
				return Ok(());
			}
			_ => return Ok(()),
		};

		let view = texture
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());
		let mut encoder = handle
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
		self.surface.blitter.copy(
			&handle.device,
			&mut encoder,
			&self.surface.target_view,
			&view,
		);
		handle.queue.submit([encoder.finish()]);
		handle.queue.present(texture);
		Ok(())
	}
}

fn gpu(error: impl std::fmt::Display) -> DesktopError {
	DesktopError::Gpu(error.to_string())
}
