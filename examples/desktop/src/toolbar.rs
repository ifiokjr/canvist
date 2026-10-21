//! The toolbar: buttons that run the editor's typed commands and light up
//! when their format is active, plus a status line on the right.

use canvist_desktop::kurbo::Affine;
use canvist_desktop::kurbo::Point;
use canvist_desktop::kurbo::Rect;
use canvist_desktop::kurbo::RoundedRect;
use canvist_headless::canvist_editor::Command;
use canvist_headless::canvist_editor::EditorState;
use canvist_headless::canvist_extensions::HeadingLevel;
use canvist_headless::canvist_extensions::StarterKitHandles;
use parley::FontContext;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use parley::StyleProperty;
use vello::Scene;
use vello::peniko::Color;
use vello::peniko::Fill;

/// The toolbar's height in logical pixels.
pub const HEIGHT: f64 = 52.0;
const BUTTON_HEIGHT: f64 = 32.0;
const BUTTON_PADDING: f64 = 10.0;
const BUTTON_GAP: f64 = 4.0;
const GROUP_GAP: f64 = 14.0;
const EDGE: f64 = 12.0;
const LABEL_SIZE: f32 = 13.0;
/// Between regular and semibold: labels read as controls, not text.
const LABEL_WEIGHT: f32 = 560.0;
const BUTTON_RADIUS: f64 = 8.0;

const BAR: Color = Color::from_rgb8(247, 246, 243);
const RULE: Color = Color::from_rgb8(226, 223, 216);
const INK: Color = Color::from_rgb8(46, 42, 38);
const MUTED: Color = Color::from_rgb8(128, 122, 114);
const HOVER: Color = Color::from_rgb8(234, 231, 225);
const ACTIVE: Color = Color::from_rgba8(59, 44, 245, 36);
const ACCENT: Color = Color::from_rgb8(59, 44, 245);

/// Something a button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
	Open,
	Save,
	Heading(HeadingLevel),
	Bold,
	Italic,
	Strike,
	Code,
	BulletList,
	OrderedList,
	TaskList,
	Quote,
	CodeBlock,
	Undo,
	Redo,
}

/// The buttons in groups, left to right.
const GROUPS: &[&[(Tool, &str)]] = &[
	&[(Tool::Open, "Open"), (Tool::Save, "Save")],
	&[
		(Tool::Heading(HeadingLevel::H1), "H1"),
		(Tool::Heading(HeadingLevel::H2), "H2"),
	],
	&[
		(Tool::Bold, "B"),
		(Tool::Italic, "I"),
		(Tool::Strike, "S"),
		(Tool::Code, "</>"),
	],
	&[
		(Tool::BulletList, "• List"),
		(Tool::OrderedList, "1. List"),
		(Tool::TaskList, "Tasks"),
	],
	&[(Tool::Quote, "Quote"), (Tool::CodeBlock, "Code")],
	&[(Tool::Undo, "Undo"), (Tool::Redo, "Redo")],
];

impl Tool {
	/// The editor command the button runs, from the typed handles. Open and
	/// Save are the app's own.
	pub fn command(self, kit: StarterKitHandles) -> Option<Command> {
		Some(match self {
			Self::Open | Self::Save => return None,
			Self::Heading(level) => kit.heading.toggle(level),
			Self::Bold => kit.bold.toggle(),
			Self::Italic => kit.italic.toggle(),
			Self::Strike => kit.strike.toggle(),
			Self::Code => kit.code.toggle(),
			Self::BulletList => kit.bullet_list.toggle(),
			Self::OrderedList => kit.ordered_list.toggle(),
			Self::TaskList => kit.task_list.toggle(),
			Self::Quote => kit.blockquote.toggle(),
			Self::CodeBlock => kit.code_block.toggle(None),
			Self::Undo => kit.history.undo(),
			Self::Redo => kit.history.redo(),
		})
	}

	/// Whether the format the button toggles is on at the selection.
	fn is_active(self, kit: StarterKitHandles, state: &EditorState) -> bool {
		match self {
			Self::Heading(level) => kit.heading.level(state) == Some(level),
			Self::Bold => kit.bold.is_active(state),
			Self::Italic => kit.italic.is_active(state),
			Self::Strike => kit.strike.is_active(state),
			Self::Code => kit.code.is_active(state),
			Self::BulletList => kit.bullet_list.is_active(state),
			Self::OrderedList => kit.ordered_list.is_active(state),
			Self::TaskList => kit.task_list.is_active(state),
			Self::Quote => kit.blockquote.is_active(state),
			Self::CodeBlock => kit.code_block.is_active(state),
			Self::Open | Self::Save | Self::Undo | Self::Redo => false,
		}
	}
}

/// A button laid out: its tool, label, and rectangle.
#[derive(Clone, Copy)]
struct Button {
	tool: Tool,
	label: &'static str,
	rect: Rect,
}

/// The toolbar's text shaping and its laid-out buttons.
pub struct Toolbar {
	fonts: FontContext,
	layouts: LayoutContext,
	buttons: Vec<Button>,
	/// Where the pointer is, in window logical pixels.
	pub hover: Option<Point>,
}

impl Toolbar {
	pub fn new() -> Self {
		let mut toolbar = Self {
			fonts: FontContext::new(),
			layouts: LayoutContext::new(),
			buttons: Vec::new(),
			hover: None,
		};
		toolbar.lay_out();
		toolbar
	}

	/// The tool under `point`, if any.
	pub fn tool_at(&self, point: Point) -> Option<Tool> {
		self.buttons
			.iter()
			.find(|button| button.rect.contains(point))
			.map(|button| button.tool)
	}

	/// Draw the bar `width` logical pixels wide at `scale`, lighting up
	/// active formats, with `status` on the right.
	pub fn paint(
		&mut self,
		scene: &mut Scene,
		scale: f64,
		width: f64,
		active: impl Fn(Tool) -> bool,
		status: &str,
	) {
		let to_window = Affine::scale(scale);
		let bar = Rect::new(0.0, 0.0, width, HEIGHT);
		let rule = Rect::new(0.0, HEIGHT - 1.0, width, HEIGHT);
		scene.fill(Fill::NonZero, to_window, BAR, None, &bar);
		scene.fill(Fill::NonZero, to_window, RULE, None, &rule);

		for index in 0..self.buttons.len() {
			let on = active(self.buttons[index].tool);
			self.paint_button(scene, to_window, index, on);
		}

		// The status only shows where it fits beside the buttons.
		let buttons_end = self.buttons.last().map_or(0.0, |button| button.rect.x1);

		if width - EDGE - self.measure(status) > buttons_end + GROUP_GAP {
			let label = Label {
				text: status,
				anchor: Point::new(width - EDGE, HEIGHT / 2.0),
				color: MUTED,
				align: Align::Right,
			};
			self.text(scene, to_window, &label);
		}
	}

	/// Draw button `index`: filled when `on`, tinted under the pointer.
	fn paint_button(&mut self, scene: &mut Scene, to_window: Affine, index: usize, on: bool) {
		let Button { label, rect, .. } = self.buttons[index];
		let hovered = self.hover.is_some_and(|point| rect.contains(point));
		let background = if on {
			Some(ACTIVE)
		} else {
			hovered.then_some(HOVER)
		};

		if let Some(color) = background {
			let shape = RoundedRect::from_rect(rect, BUTTON_RADIUS);
			scene.fill(Fill::NonZero, to_window, color, None, &shape);
		}

		let label = Label {
			text: label,
			anchor: rect.center(),
			color: if on { ACCENT } else { INK },
			align: Align::Center,
		};
		self.text(scene, to_window, &label);
	}

	/// Lay the buttons out left to right, sized to their labels.
	fn lay_out(&mut self) {
		let mut x = EDGE;
		let top = (HEIGHT - BUTTON_HEIGHT) / 2.0;
		let mut buttons = Vec::new();

		for group in GROUPS {
			for &(tool, label) in *group {
				let width = self.measure(label) + BUTTON_PADDING * 2.0;
				buttons.push(Button {
					tool,
					label,
					rect: Rect::new(x, top, x + width, top + BUTTON_HEIGHT),
				});
				x += width + BUTTON_GAP;
			}

			x += GROUP_GAP - BUTTON_GAP;
		}

		self.buttons = buttons;
	}

	fn measure(&mut self, text: &str) -> f64 {
		f64::from(self.layout(text).width())
	}

	fn layout(&mut self, text: &str) -> parley::Layout<[u8; 4]> {
		let mut builder = self
			.layouts
			.ranged_builder(&mut self.fonts, text, 1.0, true);
		builder.push_default(StyleProperty::FontFamily(parley::FontFamily::Source(
			"system-ui, sans-serif".into(),
		)));
		builder.push_default(StyleProperty::FontSize(LABEL_SIZE));
		builder.push_default(StyleProperty::FontWeight(parley::FontWeight::new(
			LABEL_WEIGHT,
		)));
		let mut layout = builder.build(text);
		layout.break_all_lines(None);
		layout
	}

	/// Draw a label vertically centred on its anchor.
	fn text(&mut self, scene: &mut Scene, to_window: Affine, label: &Label<'_>) {
		let layout = self.layout(label.text);
		let width = f64::from(layout.width());
		let left = match label.align {
			Align::Center => label.anchor.x - width / 2.0,
			Align::Right => label.anchor.x - width,
		};
		let top = label.anchor.y - f64::from(layout.height()) / 2.0;
		let origin = to_window * Affine::translate((left, top));
		let runs = layout
			.lines()
			.flat_map(|line| line.items())
			.filter_map(|item| {
				match item {
					PositionedLayoutItem::GlyphRun(run) => Some(run),
					PositionedLayoutItem::InlineBox(_) => None,
				}
			});

		for run in runs {
			let glyphs = run.positioned_glyphs().map(|glyph| {
				vello::Glyph {
					id: glyph.id,
					x: glyph.x,
					y: glyph.y,
				}
			});
			scene
				.draw_glyphs(run.run().font())
				.font_size(run.run().font_size())
				.transform(origin)
				.brush(label.color)
				.draw(Fill::NonZero, glyphs);
		}
	}
}

/// Text to draw on the bar, and where.
struct Label<'a> {
	text: &'a str,
	anchor: Point,
	color: Color,
	align: Align,
}

#[derive(Clone, Copy)]
enum Align {
	Center,
	Right,
}

/// Whether `tool` should light up for `state`.
pub fn active(tool: Tool, kit: StarterKitHandles, state: &EditorState) -> bool {
	tool.is_active(kit, state)
}
