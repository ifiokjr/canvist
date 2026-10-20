//! The GPU renderer draws what the CPU renderer draws: the same frame,
//! rendered offscreen with vello on wgpu and with `vello_cpu`, compared
//! pixel by pixel. Machines without a GPU adapter (most CI runners) skip
//! it with a message.

#![cfg(feature = "winit")]

mod support;

use canvist_desktop::InputEvent;
use canvist_desktop::Viewport;
use kurbo::Affine;
use support::Driver;
use vello::AaConfig;
use vello::RenderParams;
use vello::Renderer;
use vello::RendererOptions;
use vello::util::RenderContext;
use vello::wgpu;

/// Bold, italic, a list, code, and a task: every kind of glyph run and
/// shape the editor paints, including synthetic bold and italic (the
/// bundled font has neither).
const DOCUMENT: &str = "# Shapes *and* **weights**

Plain, *slanted*, **heavy**, and `code`.

- [x] done
- [ ] next";
/// Channel difference the two rasterisers may have on a pixel: they
/// antialias edges differently, by up to about half a pixel's coverage.
const CHANNEL_SLACK: u8 = 160;
/// Share of pixels allowed past that. Matching frames differ on about 0.08%
/// of pixels and glyphs slanted the wrong way on 0.36%, so this sits between.
const DIFFERENT_SHARE: f64 = 0.002;

#[test]
fn the_gpu_paints_what_the_cpu_paints() {
	let Some(gpu) = render_on_gpu() else {
		eprintln!("skipped: no GPU adapter");

		return;
	};
	let driver = driver();
	let (_, _, cpu) = driver.editor.render_rgba8().expect("cpu pixels");

	if let Ok(path) = std::env::var("CANVIST_GPU_PNG") {
		write_png(&path, &driver, &gpu);
	}

	let different = cpu
		.chunks_exact(4)
		.zip(gpu.chunks_exact(4))
		.filter(|(a, b)| {
			a.iter()
				.zip(b.iter())
				.any(|(a, b)| a.abs_diff(*b) > CHANNEL_SLACK)
		})
		.count();
	let allowed = (cpu.len() / 4) as f64 * DIFFERENT_SHARE;
	eprintln!("{different} pixels differ (allowed {allowed:.0})");

	assert!(
		different as f64 <= allowed,
		"{different} pixels differ between the GPU and CPU renderers (allowed {allowed:.0})"
	);
}

fn driver() -> Driver {
	let mut driver = Driver::sized(DOCUMENT, 480.0, 260.0);
	let _ = driver.send(&InputEvent::Focus(false));
	driver.editor.resize(Viewport {
		width: 480.0,
		height: 260.0,
		scale: 2.0,
	});
	driver.frame();
	driver
}

/// The frame rendered offscreen by vello, as RGBA8 rows, or `None` when
/// this machine has no adapter.
fn render_on_gpu() -> Option<Vec<u8>> {
	let driver = driver();
	let (width, height) = driver.editor.pixel_size();
	let mut painter = canvist_paint::VelloPainter::new();
	*painter.shaper_mut() = driver.editor.fonts().shaper();
	driver.editor.paint(
		&mut painter,
		Affine::scale(f64::from(driver.editor.viewport().scale)),
	);

	let mut context = RenderContext::new();
	let id = pollster::block_on(context.device(None))?;
	let handle = &context.devices[id];
	let (device, queue) = (&handle.device, &handle.queue);
	let mut renderer = Renderer::new(
		device,
		RendererOptions {
			antialiasing_support: vello::AaSupport::area_only(),
			..RendererOptions::default()
		},
	)
	.ok()?;
	let texture = device.create_texture(&wgpu::TextureDescriptor {
		label: None,
		size: wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8Unorm,
		usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
		view_formats: &[],
	});
	let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
	let background =
		canvist_paint::convert::layout_color(driver.editor.options().theme.chrome().background);
	renderer
		.render_to_texture(
			device,
			queue,
			painter.scene(),
			&view,
			&RenderParams {
				base_color: background,
				width,
				height,
				antialiasing_method: AaConfig::Area,
			},
		)
		.ok()?;

	Some(read_back(device, queue, &texture, width, height))
}

/// Copy `texture` to the CPU, dropping the row padding wgpu requires.
fn read_back(
	device: &wgpu::Device,
	queue: &wgpu::Queue,
	texture: &wgpu::Texture,
	width: u32,
	height: u32,
) -> Vec<u8> {
	let row = width * 4;
	let padded =
		row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
	let buffer = device.create_buffer(&wgpu::BufferDescriptor {
		label: None,
		size: u64::from(padded * height),
		usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
		mapped_at_creation: false,
	});
	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
	encoder.copy_texture_to_buffer(
		texture.as_image_copy(),
		wgpu::TexelCopyBufferInfo {
			buffer: &buffer,
			layout: wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(padded),
				rows_per_image: None,
			},
		},
		wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		},
	);
	queue.submit([encoder.finish()]);

	let slice = buffer.slice(..);
	let (sender, receiver) = std::sync::mpsc::channel();
	slice.map_async(wgpu::MapMode::Read, move |result| {
		let _ = sender.send(result);
	});
	device
		.poll(wgpu::PollType::wait_indefinitely())
		.expect("the copy finished");
	receiver
		.recv()
		.expect("the map callback ran")
		.expect("the buffer mapped");

	let data = slice.get_mapped_range().expect("the mapped range");
	data.chunks(padded as usize)
		.flat_map(|line| &line[..row as usize])
		.copied()
		.collect()
}

fn write_png(path: &str, driver: &Driver, pixels: &[u8]) {
	let (width, height) = driver.editor.pixel_size();
	let file = std::fs::File::create(path).expect("png file");
	let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
	encoder.set_color(png::ColorType::Rgba);
	encoder.set_depth(png::BitDepth::Eight);
	encoder
		.write_header()
		.and_then(|mut writer| writer.write_image_data(pixels))
		.expect("png");
}
