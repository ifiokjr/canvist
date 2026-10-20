//! The select tool: click and shift-click to select, drag to move with
//! snapping, drag handles to resize, rotate, and skew, drag on empty canvas
//! to marquee, and keys to delete, nudge, and deselect.

use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use kurbo::BezPath;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Shape as _;
use kurbo::Vec2;

use super::HIT_TOLERANCE;
use super::Outcome;
use super::Press;
use super::Tool;
use super::is_primary_down;
use super::pointer;
use crate::CanvasError;
use crate::CanvasState;
use crate::Cursor;
use crate::Frame;
use crate::Handle;
use crate::HandleStyle;
use crate::Input;
use crate::OverlayItem;
use crate::geometry::closed_polygon;
use crate::handles;
use crate::hit;
use crate::input::KeyEvent;
use crate::input::Phase;
use crate::input::PointerEvent;
use crate::overlay::OutlineKind;
use crate::snap::Axis;
use crate::snap::Snapper;
use crate::transform::Originals;

/// Screen pixels within which a moving selection snaps.
const SNAP_PIXELS: f64 = 6.0;
/// Seconds between two clicks that make a double click.
const DOUBLE_CLICK: f64 = 0.35;
/// World units an arrow key nudges by, or ten times that with Shift.
const NUDGE: f64 = 1.0;
/// Screen pixels between the bottom of the selection and its size label.
const LABEL_GAP: f64 = 16.0;

/// What the select tool is doing.
#[derive(Debug, Default)]
enum Gesture {
	#[default]
	Idle,
	/// Pressed but not yet dragged.
	Pressing {
		press: Press,
		target: Option<NodeId>,
		toggled: bool,
	},
	/// Moving the selection.
	Moving {
		press: Press,
		originals: Originals,
		bounds: Rect,
		snapper: Snapper,
		merge: bool,
	},
	/// Dragging a handle.
	Transforming {
		press: Press,
		handle: Handle,
		originals: Originals,
		frame: Frame,
		merge: bool,
	},
	/// Drawing a marquee.
	Marquee {
		press: Press,
		base: Vec<NodeId>,
		current: Point,
	},
}

/// The select tool.
#[derive(Debug, Default)]
pub struct SelectTool {
	gesture: Gesture,
	hover: Option<NodeId>,
	hover_handle: Option<Handle>,
	last_click: Option<(f64, NodeId)>,
	style: HandleStyle,
}

impl SelectTool {
	/// A select tool with default handle sizes.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	fn down(&mut self, canvas: &mut CanvasState, event: &PointerEvent) -> Outcome {
		let frame = canvas.selection().frame(canvas.index());
		let handle = frame.and_then(|frame| {
			handles::handle_at(&frame, canvas.camera(), event.position, &self.style)
		});

		if let (Some(frame), Some(handle)) = (frame, handle)
			&& handle != Handle::Body
		{
			let originals =
				Originals::capture(canvas.scene(), canvas.index(), canvas.selection().nodes());
			self.gesture = Gesture::Transforming {
				press: Press::new(event.position),
				handle,
				originals,
				frame,
				merge: false,
			};

			return Outcome::Redraw;
		}

		let target = Self::target_at(canvas, event);

		if let Some(target) = target
			&& self.is_double_click(event.time, target)
			&& canvas.scene().kind(target) == Some(NodeKind::Text)
		{
			return Outcome::EditText(target);
		}

		self.last_click = target.map(|target| (event.time, target));
		self.press(canvas, event, target, handle == Some(Handle::Body))
	}

	/// Act on a press that hit no handle: Shift toggles `target` in the
	/// selection, a press on an unselected node selects only it, and a press
	/// on empty canvas outside the selection's frame starts a marquee (added
	/// to the selection with Shift). Anything else waits to see whether the
	/// press becomes a move.
	fn press(
		&mut self,
		canvas: &mut CanvasState,
		event: &PointerEvent,
		target: Option<NodeId>,
		in_frame: bool,
	) -> Outcome {
		let shift = event.modifiers.shift;

		match target {
			Some(target) if shift => canvas.selection_mut().toggle(target),

			Some(target) if !canvas.selection().contains(target) => {
				canvas.selection_mut().set([target]);
			}

			None if in_frame => {}
			None => return self.start_marquee(canvas, event),
			Some(_) => {}
		}

		self.gesture = Gesture::Pressing {
			press: Press::new(event.position),
			target,
			toggled: shift,
		};
		Outcome::Redraw
	}

	/// Start a marquee at the press; with Shift it adds to the selection,
	/// otherwise it replaces it.
	fn start_marquee(&mut self, canvas: &mut CanvasState, event: &PointerEvent) -> Outcome {
		let base = if event.modifiers.shift {
			canvas.selection().nodes().to_vec()
		} else {
			Vec::new()
		};

		canvas.selection_mut().set(base.clone());
		self.gesture = Gesture::Marquee {
			press: Press::new(event.position),
			base,
			current: event.position,
		};
		Outcome::Redraw
	}

	fn target_at(canvas: &CanvasState, event: &PointerEvent) -> Option<NodeId> {
		let world = canvas.camera().to_world(event.position);
		let tolerance = HIT_TOLERANCE / canvas.camera().zoom;
		let hit = hit::hit_test(canvas.scene(), canvas.index(), world, tolerance)?;
		Some(hit::selection_target(
			canvas.scene(),
			hit,
			event.modifiers.primary,
		))
	}

	fn is_double_click(&self, time: f64, target: NodeId) -> bool {
		self.last_click
			.is_some_and(|(previous, node)| node == target && time - previous <= DOUBLE_CLICK)
	}

	fn drag(
		&mut self,
		canvas: &mut CanvasState,
		event: &PointerEvent,
	) -> Result<Outcome, CanvasError> {
		match &mut self.gesture {
			Gesture::Pressing { press, .. } => {
				let press = *press;
				self.drag_pressed(canvas, event, press)
			}
			Gesture::Moving { .. } => self.move_selection(canvas, event),
			Gesture::Transforming { .. } => self.transform(canvas, event),
			Gesture::Marquee {
				press,
				base,
				current,
			} => {
				press.moved(event.position);
				*current = event.position;
				let camera = canvas.camera();
				let rect =
					Rect::from_points(camera.to_world(press.start), camera.to_world(*current));
				let picked = hit::marquee(
					canvas.scene(),
					canvas.index(),
					rect,
					hit::MarqueeMode::Intersect,
				);
				let base = base.clone();
				canvas.selection_mut().set(base.into_iter().chain(picked));
				Ok(Outcome::Redraw)
			}
			Gesture::Idle => Ok(Outcome::Idle),
		}
	}

	/// Turn a press into a move once the pointer travels past the drag
	/// threshold.
	fn drag_pressed(
		&mut self,
		canvas: &mut CanvasState,
		event: &PointerEvent,
		press: Press,
	) -> Result<Outcome, CanvasError> {
		if !Press::new(press.start).moved(event.position) {
			return Ok(Outcome::Idle);
		}

		self.start_move(canvas, press);
		self.move_selection(canvas, event)
	}

	fn start_move(&mut self, canvas: &CanvasState, press: Press) {
		let nodes = canvas.selection().nodes().to_vec();
		let originals = Originals::capture(canvas.scene(), canvas.index(), &nodes);
		let bounds = canvas.index().bounds_of(&nodes).unwrap_or_default();
		let area = canvas.camera().visible(canvas.viewport());
		let snapper = Snapper::new(canvas.scene(), canvas.index(), &nodes, area);

		self.gesture = Gesture::Moving {
			press: Press {
				dragging: true,
				..press
			},
			originals,
			bounds,
			snapper,
			merge: false,
		};
	}

	fn move_selection(
		&mut self,
		canvas: &mut CanvasState,
		event: &PointerEvent,
	) -> Result<Outcome, CanvasError> {
		let Gesture::Moving {
			press,
			originals,
			bounds,
			snapper,
			merge,
		} = &mut self.gesture
		else {
			return Ok(Outcome::Idle);
		};
		let camera = *canvas.camera();
		let mut offset = camera.to_world(event.position) - camera.to_world(press.start);
		canvas.chrome.clear();

		if !event.modifiers.primary {
			let snap = snapper.snap(*bounds + offset, SNAP_PIXELS / camera.zoom);
			offset += snap.offset;
			canvas
				.chrome
				.extend(snap.guides.iter().map(|guide| guide_item(&camera, guide)));
		}

		let steps = originals.translate(canvas.scene(), offset);
		canvas.apply(steps, *merge)?;
		*merge = true;
		Ok(Outcome::Redraw)
	}

	fn transform(
		&mut self,
		canvas: &mut CanvasState,
		event: &PointerEvent,
	) -> Result<Outcome, CanvasError> {
		let Gesture::Transforming {
			press,
			handle,
			originals,
			frame,
			merge,
		} = &mut self.gesture
		else {
			return Ok(Outcome::Idle);
		};
		press.moved(event.position);
		let camera = canvas.camera();
		let (start, current) = (
			camera.to_world(press.start),
			camera.to_world(event.position),
		);
		let modifiers = event.modifiers;

		let next = match *handle {
			Handle::Rotate(_) => {
				let snap = modifiers.shift.then_some(handles::ROTATION_SNAP);
				handles::rotate(frame, start, current, snap).0
			}

			Handle::Edge(edge) if modifiers.primary => handles::skew(frame, edge, start, current),
			other => {
				let options = handles::ResizeOptions {
					keep_aspect: modifiers.shift,
					from_center: modifiers.alt,
				};
				handles::resize(frame, other, start, current, options)
			}
		};

		let steps = originals.steps(canvas.scene(), frame, &next);
		canvas.apply(steps, *merge)?;
		*merge = true;
		Ok(Outcome::Redraw)
	}

	fn up(&mut self, canvas: &mut CanvasState) -> Outcome {
		let gesture = std::mem::take(&mut self.gesture);
		canvas.chrome.clear();

		if let Gesture::Pressing {
			press,
			target: Some(target),
			toggled: false,
		} = gesture
			&& !press.dragging
			&& canvas.selection().nodes().len() > 1
		{
			canvas.selection_mut().set([target]);
		}

		Outcome::Redraw
	}

	fn hover(&mut self, canvas: &CanvasState, event: &PointerEvent) -> Outcome {
		let frame = canvas.selection().frame(canvas.index());
		let handle = frame.and_then(|frame| {
			handles::handle_at(&frame, canvas.camera(), event.position, &self.style)
		});
		let hover = Self::target_at(canvas, event);
		let changed = hover != self.hover || handle != self.hover_handle;
		self.hover = hover;
		self.hover_handle = handle;

		if changed {
			Outcome::Redraw
		} else {
			Outcome::Idle
		}
	}

	fn key(canvas: &mut CanvasState, event: &KeyEvent) -> Result<Outcome, CanvasError> {
		if !event.down || canvas.selection().is_empty() {
			return Ok(Outcome::Idle);
		}

		let step = if event.modifiers.shift {
			NUDGE * 10.0
		} else {
			NUDGE
		};

		let nudge = match event.key.as_str() {
			"ArrowLeft" => Vec2::new(-step, 0.0),
			"ArrowRight" => Vec2::new(step, 0.0),
			"ArrowUp" => Vec2::new(0.0, -step),
			"ArrowDown" => Vec2::new(0.0, step),
			"Backspace" | "Delete" => {
				let nodes = canvas.selection().nodes().to_vec();
				canvas.edit(false, |transaction| transaction.remove(&nodes))?;
				canvas.selection_mut().clear();
				return Ok(Outcome::Redraw);
			}
			"Escape" => {
				canvas.selection_mut().clear();
				return Ok(Outcome::Redraw);
			}
			_ => return Ok(Outcome::Idle),
		};

		let originals =
			Originals::capture(canvas.scene(), canvas.index(), canvas.selection().nodes());
		let steps = originals.translate(canvas.scene(), nudge);

		canvas.apply(steps, false)?;
		Ok(Outcome::Redraw)
	}
}

impl Tool for SelectTool {
	fn name(&self) -> &'static str {
		"select"
	}

	fn input(&mut self, canvas: &mut CanvasState, input: &Input) -> Result<Outcome, CanvasError> {
		if let Input::Key(event) = input {
			return Self::key(canvas, event);
		}

		let Some(event) = pointer(input) else {
			return Ok(Outcome::Idle);
		};

		match event.phase {
			_ if is_primary_down(event) => Ok(self.down(canvas, event)),

			Phase::Move if event.buttons & 1 == 1 => self.drag(canvas, event),
			Phase::Move => Ok(self.hover(canvas, event)),
			Phase::Up | Phase::Cancel => Ok(self.up(canvas)),
			Phase::Down => Ok(Outcome::Idle),
		}
	}

	fn overlay(&self, canvas: &CanvasState, out: &mut Vec<OverlayItem>) {
		let camera = canvas.camera();

		if let Some(hover) = self.hover.filter(|id| !canvas.selection().contains(*id))
			&& let Some(path) = screen_outline(canvas, hover)
		{
			out.push(OverlayItem::Outline {
				path,
				kind: OutlineKind::Hover,
			});
		}

		if let Gesture::Marquee { press, current, .. } = &self.gesture {
			out.push(OverlayItem::Marquee {
				rect: Rect::from_points(press.start, *current),
			});
		}

		let Some(frame) = canvas.selection().frame(canvas.index()) else {
			return;
		};
		let corners = frame.corners().map(|corner| camera.to_screen(corner));
		out.push(OverlayItem::Outline {
			path: closed_polygon(corners),
			kind: OutlineKind::Frame,
		});
		out.extend(
			handles::squares(&frame, camera, &self.style)
				.into_iter()
				.map(|(handle, rect)| OverlayItem::Handle { rect, handle }),
		);
		// Node transforms carry no scale, so the frame's axes have the
		// selection's width and height.
		let [a, b, c, d, _, _] = frame.transform.as_coeffs();
		let bottom = corners
			.iter()
			.map(|corner| corner.y)
			.fold(f64::MIN, f64::max);
		out.push(OverlayItem::Label {
			at: Point::new(camera.to_screen(frame.center()).x, bottom + LABEL_GAP),
			text: format!("{} × {}", round(a.hypot(b)), round(c.hypot(d))),
		});
	}

	fn cursor(&self, canvas: &CanvasState) -> Cursor {
		let handle = match &self.gesture {
			Gesture::Moving { .. } => return Cursor::Grabbing,
			Gesture::Transforming { handle, .. } => Some(*handle),
			_ => self.hover_handle.filter(|handle| *handle != Handle::Body),
		};

		handle.map_or(Cursor::Default, |handle| handle_cursor(canvas, handle))
	}

	fn cancel(&mut self, canvas: &mut CanvasState) {
		self.gesture = Gesture::Idle;
		canvas.chrome.clear();
	}
}

/// The cursor for a handle: resize arrows along the handle's direction from
/// the frame's centre, or a rotation cursor.
fn handle_cursor(canvas: &CanvasState, handle: Handle) -> Cursor {
	let Some(frame) = canvas.selection().frame(canvas.index()) else {
		return Cursor::Default;
	};
	let camera = canvas.camera();
	let unit = handle.unit();
	let direction =
		camera.to_screen(frame.point(unit.x, unit.y)) - camera.to_screen(frame.center());
	let angle = direction.atan2();

	match handle {
		Handle::Rotate(_) => Cursor::Rotate(angle),
		_ => Cursor::Resize(angle),
	}
}

fn guide_item(camera: &crate::Camera, guide: &crate::snap::Guide) -> OverlayItem {
	let (from, to) = match guide.axis {
		Axis::X => {
			(
				Point::new(guide.position, guide.from),
				Point::new(guide.position, guide.to),
			)
		}
		Axis::Y => {
			(
				Point::new(guide.from, guide.position),
				Point::new(guide.to, guide.position),
			)
		}
	};

	OverlayItem::Guide {
		from: camera.to_screen(from),
		to: camera.to_screen(to),
	}
}

/// A node's outline in screen space.
pub(crate) fn screen_outline(canvas: &CanvasState, id: NodeId) -> Option<BezPath> {
	let geometry = canvas.index().get(id)?;
	let to_screen = canvas.camera().world_to_screen() * geometry.world;

	if let Some(outline) = &geometry.outline {
		Some(to_screen * (**outline).clone())
	} else {
		let bounds = geometry.extent;
		Some(canvas.camera().world_to_screen() * bounds.to_path(0.1))
	}
}

fn round(value: f64) -> f64 {
	(value * 100.0).round() / 100.0
}
