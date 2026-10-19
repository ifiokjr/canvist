//! Putting frames on a `<canvas>`: vello on WebGPU where the browser has
//! it, `vello_cpu` and `putImageData` everywhere else.
//!
//! vello renders into a storage texture, which a blit copies to the
//! canvas's surface: browsers do not offer storage binding on canvas
//! textures. The CPU path rasterises into an RGBA buffer and hands it to
//! Canvas2D. Both paint the same [`App::paint`] calls.

use canvist_paint::CpuPainter;
use canvist_paint::Quality;
use canvist_paint::VelloPainter;
use vello::AaConfig;
use vello::AaSupport;
use vello::RenderParams;
use vello::Renderer;
use vello::RendererOptions;
use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;
use web_sys::HtmlCanvasElement;
use web_sys::ImageData;
use wgpu::CurrentSurfaceTexture;

use crate::App;

/// A frame size in device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pixels {
	/// The width.
	pub width: u32,
	/// The height.
	pub height: u32,
}

/// Why rendering could not start or a frame failed.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
	/// The browser has no usable WebGPU adapter or device.
	#[error("WebGPU is unavailable: {0}")]
	NoGpu(String),
	/// vello could not build its pipelines or render.
	#[error("vello failed: {0}")]
	Vello(#[from] vello::Error),
	/// The CPU rasteriser rejected the frame size.
	#[error(transparent)]
	Cpu(#[from] canvist_paint::CpuError),
	/// A Canvas2D call failed.
	#[error("Canvas2D failed: {0}")]
	Canvas(String),
}

impl From<JsValue> for RenderError {
	fn from(value: JsValue) -> Self {
		Self::Canvas(format!("{value:?}"))
	}
}

/// Puts frames on one canvas.
pub enum Backend {
	/// vello on WebGPU.
	Gpu(Box<GpuRenderer>),
	/// `vello_cpu` and Canvas2D.
	Cpu(Box<CpuRenderer>),
}

impl Backend {
	/// A renderer for `canvas`: WebGPU when `prefer_gpu` is set and the
	/// browser supports it, otherwise the CPU.
	pub async fn new(canvas: &HtmlCanvasElement, prefer_gpu: bool) -> Result<Self, RenderError> {
		if prefer_gpu {
			match GpuRenderer::new(canvas).await {
				Ok(gpu) => return Ok(Self::Gpu(Box::new(gpu))),
				Err(error) => {
					web_sys::console::warn_1(&format!("{error}; drawing on the CPU").into());
				}
			}
		}

		Ok(Self::Cpu(Box::new(CpuRenderer::new(canvas)?)))
	}

	/// `"webgpu"` or `"cpu"`.
	#[must_use]
	pub fn backend(&self) -> &'static str {
		match self {
			Self::Gpu(_) => "webgpu",
			Self::Cpu(_) => "cpu",
		}
	}

	/// Paint `app` and present it, `size` device pixels at `scale` device
	/// pixels per CSS pixel.
	pub fn render(
		&mut self,
		app: &App,
		size: Pixels,
		scale: f64,
		quality: Quality,
	) -> Result<(), RenderError> {
		if size.width == 0 || size.height == 0 {
			return Ok(());
		}

		match self {
			Self::Gpu(gpu) => gpu.render(app, size, scale, quality),
			Self::Cpu(cpu) => cpu.render(app, size, scale, quality),
		}
	}
}

/// vello on WebGPU.
pub struct GpuRenderer {
	device: wgpu::Device,
	queue: wgpu::Queue,
	surface: wgpu::Surface<'static>,
	config: wgpu::SurfaceConfiguration,
	renderer: Renderer,
	painter: VelloPainter,
	blitter: wgpu::util::TextureBlitter,
	target: Option<(Pixels, wgpu::TextureView)>,
}

impl GpuRenderer {
	async fn new(canvas: &HtmlCanvasElement) -> Result<Self, RenderError> {
		let gpu_error = |error: &dyn std::fmt::Display| RenderError::NoGpu(error.to_string());
		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: wgpu::Backends::BROWSER_WEBGPU,
			..wgpu::InstanceDescriptor::new_without_display_handle()
		});
		// Find a device before touching the canvas: once it has a WebGPU
		// context it can never have a 2D one, and the CPU fallback needs
		// that. Every WebGPU adapter can present to a canvas.
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::HighPerformance,
				..wgpu::RequestAdapterOptions::default()
			})
			.await
			.map_err(|error| gpu_error(&error))?;
		let (device, queue) = adapter
			.request_device(&wgpu::DeviceDescriptor {
				label: Some("canvist canvas"),
				required_limits: adapter.limits(),
				..wgpu::DeviceDescriptor::default()
			})
			.await
			.map_err(|error| gpu_error(&error))?;
		let surface = instance
			.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
			.map_err(|error| gpu_error(&error))?;
		let mut config = surface
			.get_default_config(&adapter, canvas.width().max(1), canvas.height().max(1))
			.ok_or_else(|| RenderError::NoGpu("the canvas surface has no formats".into()))?;
		// The blit writes the colours vello computed; an sRGB view would
		// encode them a second time.
		config.format = config.format.remove_srgb_suffix();
		surface.configure(&device, &config);

		let renderer = Renderer::new(
			&device,
			RendererOptions {
				use_cpu: false,
				antialiasing_support: AaSupport::area_only(),
				num_init_threads: None,
				pipeline_cache: None,
			},
		)?;
		let blitter = wgpu::util::TextureBlitter::new(&device, config.format);
		let mut painter = VelloPainter::new();
		painter
			.shaper_mut()
			.register_font(canvist_paint::BUNDLED_FONT.to_vec());

		Ok(Self {
			device,
			queue,
			surface,
			config,
			renderer,
			painter,
			blitter,
			target: None,
		})
	}

	fn render(
		&mut self,
		app: &App,
		size: Pixels,
		scale: f64,
		quality: Quality,
	) -> Result<(), RenderError> {
		self.painter.reset();
		app.paint(&mut self.painter, scale, quality);

		if (self.config.width, self.config.height) != (size.width, size.height) {
			self.config.width = size.width;
			self.config.height = size.height;
			self.surface.configure(&self.device, &self.config);
		}

		let target = self.target(size);
		self.renderer.render_to_texture(
			&self.device,
			&self.queue,
			self.painter.scene(),
			&target,
			&RenderParams {
				// Pages paint their own background over everything visible.
				base_color: canvist_paint::peniko::Color::TRANSPARENT,
				width: size.width,
				height: size.height,
				antialiasing_method: AaConfig::Area,
			},
		)?;

		let (CurrentSurfaceTexture::Success(frame) | CurrentSurfaceTexture::Suboptimal(frame)) =
			self.surface.get_current_texture()
		else {
			// The browser skipped this frame; reconfigure and draw the next.
			self.surface.configure(&self.device, &self.config);

			return Ok(());
		};
		let view = frame
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());
		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
		self.blitter
			.copy(&self.device, &mut encoder, &target, &view);
		self.queue.submit([encoder.finish()]);
		self.queue.present(frame);
		Ok(())
	}

	/// The storage texture vello renders into, made again when the size
	/// changes.
	fn target(&mut self, size: Pixels) -> wgpu::TextureView {
		if let Some((current, view)) = &self.target
			&& *current == size
		{
			return view.clone();
		}

		let texture = self.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("canvist canvas target"),
			size: wgpu::Extent3d {
				width: size.width,
				height: size.height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});
		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		self.target = Some((size, view.clone()));
		view
	}
}

/// `vello_cpu` and Canvas2D.
pub struct CpuRenderer {
	context: CanvasRenderingContext2d,
	painter: CpuPainter,
}

impl CpuRenderer {
	fn new(canvas: &HtmlCanvasElement) -> Result<Self, RenderError> {
		let context = canvas
			.get_context("2d")?
			.ok_or_else(|| RenderError::Canvas("the canvas has no 2d context".into()))?
			.dyn_into::<CanvasRenderingContext2d>()
			.map_err(|_| RenderError::Canvas("the 2d context has the wrong type".into()))?;
		let mut painter = CpuPainter::new(canvas.width(), canvas.height())?;
		painter
			.shaper_mut()
			.register_font(canvist_paint::BUNDLED_FONT.to_vec());

		Ok(Self { context, painter })
	}

	fn render(
		&mut self,
		app: &App,
		size: Pixels,
		scale: f64,
		quality: Quality,
	) -> Result<(), RenderError> {
		self.painter.reset(size.width, size.height)?;

		app.paint(&mut self.painter, scale, quality);
		// The background is opaque, so premultiplied and straight alpha
		// agree and the buffer goes to Canvas2D as it is.
		let pixels = self.painter.render_rgba8();
		let image =
			ImageData::new_with_u8_clamped_array_and_sh(Clamped(&pixels), size.width, size.height)?;
		self.context.put_image_data(&image, 0.0, 0.0)?;
		Ok(())
	}
}
