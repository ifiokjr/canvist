//! Each feature's modules, reached only through `canvist`.
//!
//! Every test is gated on the features it needs, so
//! `cargo hack --each-feature test -p canvist` runs each one in the smallest
//! build that should have it.

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The model, editor, and extensions are in every build.
#[test]
fn core_modules_are_always_available() -> TestResult {
	let mut builder = canvist::core::Editor::builder();
	builder.add(canvist::extensions::Document);
	builder.add(canvist::extensions::Paragraph);
	let editor = builder.build()?;

	let schema: &canvist::model::Schema = editor.schema();
	assert!(schema.block_type("paragraph").is_some());
	Ok(())
}

#[cfg(feature = "starter-kit")]
#[test]
fn starter_kit_registers_on_the_editor() -> TestResult {
	let mut builder = canvist::core::Editor::builder();
	canvist::extensions::StarterKit::add_all(&mut builder);
	let editor = builder.build()?;

	assert!(editor.schema().mark_type("bold").is_some());
	assert!(editor.schema().block_type("taskList").is_some());
	Ok(())
}

#[cfg(all(feature = "headless", feature = "starter-kit"))]
#[test]
fn headless_parses_selects_runs_and_exports() -> TestResult {
	use canvist::headless::Engine;
	use canvist::headless::Format;
	use canvist::headless::Position;

	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = engine.parse(Format::Markdown, "Hello world")?;

	doc.select_range(&Position::new([0], 6), &Position::new([0], 11))?;
	doc.run(&kit.bold.toggle())?;

	assert_eq!(
		doc.export(Format::Html),
		"<p>Hello <strong>world</strong></p>"
	);
	Ok(())
}

/// `canvist::model` is the crate `headless` builds on, not a copy of it.
#[cfg(all(feature = "headless", feature = "starter-kit"))]
#[test]
fn headless_documents_are_model_documents() -> TestResult {
	let doc = canvist::headless::Engine::starter_kit()
		.parse(canvist::headless::Format::Markdown, "one\n\ntwo")?;
	let document: &canvist::model::Document = doc.document();

	assert_eq!(document.children(None).len(), 2);
	Ok(())
}

#[cfg(feature = "extras")]
#[test]
fn extras_register_beyond_the_starter_kit() -> TestResult {
	let editor = canvist::core::Editor::builder()
		.extensions(canvist::extensions::StarterKit::default().extensions())
		.extensions(canvist::extensions::extras())
		.build()?;

	assert!(editor.schema().mark_type("highlight").is_some());
	Ok(())
}

#[cfg(feature = "full")]
#[test]
fn full_compiles_the_emoji_table_in() {
	assert!(canvist::extensions::emoji::is_loaded());
}

#[cfg(feature = "collab")]
#[test]
fn collab_binds_a_document_to_yjs() -> TestResult {
	let mut builder = canvist::core::Editor::builder();
	builder.add(canvist::extensions::Document);
	builder.add(canvist::extensions::Paragraph);
	let editor = builder.build()?;

	let mut binding = canvist::collab::Binding::new(editor.schema().clone())?;
	binding.seed(editor.state().doc())?;

	assert!(
		!binding
			.encode_state(canvist::collab::Encoding::V1)
			.is_empty()
	);
	Ok(())
}

#[cfg(all(feature = "collab", feature = "headless", feature = "starter-kit"))]
#[test]
fn collab_makes_headless_documents_yjs_peers() -> TestResult {
	let engine = canvist::headless::Engine::starter_kit();
	let mut doc = engine.parse(canvist::headless::Format::Text, "shared")?;
	doc.share(1)?;
	let peer = engine.from_yjs(&doc.encode_state()?, 2)?;

	assert_eq!(peer.export(canvist::headless::Format::Text), "shared");
	Ok(())
}

#[cfg(all(feature = "journal", feature = "starter-kit"))]
#[test]
fn journal_records_commits() -> TestResult {
	let engine = canvist::headless::Engine::starter_kit();
	let mut doc = engine.parse(canvist::headless::Format::Text, "Hello")?;
	doc.record_commits();
	doc.select_all()?;
	doc.run(&engine.handles().bold.toggle())?;

	assert_eq!(doc.take_commits().len(), 1);
	Ok(())
}

#[cfg(all(feature = "ssr", feature = "starter-kit"))]
#[test]
fn ssr_renders_html_and_svg() -> TestResult {
	let doc = canvist::headless::Engine::starter_kit()
		.parse(canvist::headless::Format::Markdown, "# Release notes")?;
	let mut renderer = canvist::headless::Renderer::new();

	assert!(renderer.html(&doc, 640.0).html.contains("Release notes"));
	assert!(renderer.svg(&doc, 640.0)?.starts_with("<svg"));
	Ok(())
}

#[cfg(feature = "layout")]
#[test]
fn layout_lays_out_a_document() -> TestResult {
	use std::sync::Arc;

	use canvist::layout::DocumentLayout;
	use canvist::layout::FixedMeasurer;
	use canvist::layout::LayoutOptions;
	use canvist::layout::Theme;
	use canvist::model::BlockSpec;
	use canvist::model::Document;
	use canvist::model::MarkSet;
	use canvist::model::Point;
	use canvist::model::Schema;

	let schema = Arc::new(
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.build()?,
	);
	let mut doc = Document::new(schema, 1);
	let block = doc.first_text_block().ok_or("no text block")?;
	let mut transaction = doc.transaction();
	transaction.insert_text(Point::new(block, 0), "Hello, layout!", MarkSet::empty())?;
	let _commit = transaction.commit();

	let mut layout = DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(80.0),
	);
	layout.layout_all(&doc, &mut FixedMeasurer::new())?;

	assert_eq!(layout.line_count(block), Some(2));
	Ok(())
}

#[cfg(feature = "parley")]
#[test]
fn parley_measures_with_real_fonts() {
	let measurer = canvist::layout::ParleyMeasurer::new();

	assert!(format!("{measurer:?}").contains("ParleyMeasurer"));
}

#[cfg(feature = "canvas")]
#[test]
fn canvas_builds_a_scene() -> TestResult {
	let mut builder = canvist::canvas::Canvas::builder();
	builder.add(canvist::canvas::Shapes);
	let scene = canvist::scene::Scene::new(builder.schema()?, 1)?;

	assert_eq!(scene.pages().len(), 1);
	Ok(())
}

#[cfg(feature = "paint")]
#[test]
fn paint_draws_svg() {
	let svg = canvist::paint::SvgPainter::new(canvist::scene::kurbo::Size::new(10.0, 10.0));

	assert!(svg.finish().starts_with("<svg"));
}

#[cfg(feature = "png")]
#[test]
fn png_rasterises_on_the_cpu() {
	assert!(std::any::type_name::<canvist::paint::CpuPainter>().ends_with("CpuPainter"));
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_draws_with_vello() {
	assert!(std::any::type_name::<canvist::paint::VelloPainter>().ends_with("VelloPainter"));
}

#[cfg(feature = "headless-scene")]
#[test]
fn headless_scene_exports_scenes() -> TestResult {
	use canvist::headless::ExportOptions;
	use canvist::headless::SceneEngine;

	let scene = SceneEngine::design_kit().from_json(&serde_json::json!({
		"type": "canvist-scene",
		"version": 1,
		"nodes": [{
			"type": "page",
			"children": [{
				"type": "rectangle",
				"props": { "size": [40, 20], "fills": [{ "type": "solid", "color": "#3b2cf5" }] }
			}]
		}]
	}))?;

	assert!(
		scene
			.export_svg(&ExportOptions::default())?
			.starts_with("<svg")
	);
	assert!(
		scene
			.export_png(&ExportOptions::default())?
			.starts_with(b"\x89PNG")
	);
	Ok(())
}
