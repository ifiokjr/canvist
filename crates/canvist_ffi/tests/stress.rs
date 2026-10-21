//! Fast typing must never fail or lose input: thousands of random text
//! input calls, in the mix a keyboard, an input method, autocorrect, and a
//! collaborator produce, with a frame painted between bursts.

use std::sync::Arc;

use canvist_ffi::CanvistError;
use canvist_ffi::DocFormat;
use canvist_ffi::Editor;
use canvist_ffi::EditorOptions;
use canvist_ffi::Modifiers;
use canvist_ffi::OffsetRange;
use canvist_ffi::testing::FixedPlatformMeasurer;

/// A small deterministic generator, so a failure replays exactly.
struct Rng(u64);

impl Rng {
	fn next(&mut self) -> u64 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		self.0
	}

	fn below(&mut self, bound: u32) -> u32 {
		if bound == 0 {
			return 0;
		}

		(self.next() % u64::from(bound)) as u32
	}

	fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
		items[self.below(items.len() as u32) as usize]
	}
}

const WORDS: [&str; 10] = [
	"a",
	"é",
	"😀",
	"漢字",
	" ",
	"word ",
	"\n",
	"- ",
	"## ",
	"مرحبا",
];
/// Rounds of random input per seed, and seeds to run.
const ROUNDS: u32 = 1_500;
const SEEDS: u64 = 4;
/// Paint a frame, and compare with a fresh editor, this often.
const FRAME_EVERY: u32 = 25;
/// The furthest a frame scrolls, in points.
const MAX_SCROLL: u32 = 200;
/// Rounds of concurrent input, and how often (one in this many rounds) the
/// two editors exchange updates.
const CONCURRENT_ROUNDS: u32 = 600;
const EXCHANGE_ONE_IN: u32 = 4;
/// How many kinds of input [`step`] chooses between.
const INPUT_KINDS: u32 = 12;
/// The longest composing region and the furthest a marked selection sits
/// into the marked text.
const MAX_REGION: u32 = 4;
const MAX_MARKED_SELECTION: u32 = 3;
/// How many times the sentence is typed.
const SENTENCES: usize = 40;
/// Mixes the seed into a well-spread starting state.
const SEED_MIX: u64 = 0x9E37_79B9_7F4A_7C15;
const CONCURRENT_SEED: u64 = 0x2545_F491_4F6C_DD1D;

const KEYS: [&str; 7] = [
	"Backspace",
	"Delete",
	"Enter",
	"ArrowLeft",
	"ArrowRight",
	"ArrowUp",
	"ArrowDown",
];

fn editor(client: u32) -> Arc<Editor> {
	let mut options = EditorOptions::starter_kit();
	options.client = client;
	let editor =
		Editor::new(options, Arc::new(FixedPlatformMeasurer::default())).expect("valid options");
	editor.set_viewport(320.0, 480.0);
	editor.set_focused(true);
	editor
}

/// The editor's text model agrees with the document.
fn assert_consistent(editor: &Editor) {
	let text = editor.plain_text();
	let length = text.encode_utf16().count() as u32;
	assert_eq!(editor.text_length(), length);

	let selection = editor.selection();
	assert!(selection.anchor <= length && selection.head <= length);

	if let Some(marked) = editor.marked_range() {
		assert!(marked.start <= marked.end && marked.end <= length);
	}
}

/// The editor's text agrees with a fresh editor loaded from its JSON,
/// whose text index is built from scratch.
fn assert_matches_fresh(editor: &Editor) {
	let fresh = self::editor(9);
	fresh
		.load(DocFormat::Json, &editor.export(DocFormat::Json))
		.expect("an export loads");
	assert_eq!(fresh.plain_text(), editor.plain_text());
}

fn step(editor: &Editor, rng: &mut Rng) -> Result<(), CanvistError> {
	let length = editor.text_length();

	match rng.below(INPUT_KINDS) {
		0..=3 => {
			editor.insert_text(rng.pick(&WORDS))?;
		}
		4 => {
			editor.press(rng.pick(&KEYS), Modifiers::default())?;
		}
		5 => {
			editor.delete_backward()?;
		}
		6 => {
			let start = rng.below(length + 1);
			let end = start + rng.below(length - start + 1);
			editor.replace(OffsetRange { start, end }, rng.pick(&WORDS))?;
		}
		7 => {
			let selected = rng.below(MAX_MARKED_SELECTION);
			editor.set_marked_text(
				rng.pick(&WORDS),
				OffsetRange {
					start: selected,
					end: selected,
				},
			)?;
		}
		8 => {
			editor.unmark_text()?;
		}
		9 => {
			editor.select(rng.below(length + 1), rng.below(length + 1))?;
		}
		10 => {
			undo_or_redo(editor, rng)?;
		}
		_ => {
			let start = rng.below(length + 1);
			editor.set_composing_region(Some(OffsetRange {
				start,
				end: start + rng.below(MAX_REGION),
			}))?;
		}
	}

	Ok(())
}

fn undo_or_redo(editor: &Editor, rng: &mut Rng) -> Result<(), CanvistError> {
	let undo = rng.below(2) == 0;

	if undo {
		editor.undo()?;
	} else {
		editor.redo()?;
	}

	Ok(())
}

/// Send each editor's waiting updates to the other.
fn exchange(first: &Editor, second: &Editor) -> Result<(), CanvistError> {
	for update in first.take_updates()? {
		second.apply_update(update)?;
	}

	for update in second.take_updates()? {
		first.apply_update(update)?;
	}

	Ok(())
}

/// Paint a frame somewhere in the document, and check the text model
/// against one built from scratch.
fn paint_and_compare(editor: &Editor, rng: &mut Rng) -> Result<(), CanvistError> {
	editor.set_scroll(rng.below(MAX_SCROLL) as f32);
	editor.frame()?;
	assert_matches_fresh(editor);
	Ok(())
}

#[test]
fn random_input_never_fails_and_keeps_the_text_model_consistent() -> Result<(), CanvistError> {
	for seed in 1..=SEEDS {
		let editor = editor(1);
		let mut rng = Rng(seed.wrapping_mul(SEED_MIX));

		for round in 0..ROUNDS {
			step(&editor, &mut rng)?;
			assert_consistent(&editor);

			if round % FRAME_EVERY == 0 {
				paint_and_compare(&editor, &mut rng)?;
			}
		}
	}

	Ok(())
}

#[test]
fn every_keystroke_lands_when_typing_fast() -> Result<(), CanvistError> {
	let editor = editor(1);
	let typed: String = "The quick brown fox jumps over the lazy dog. "
		.repeat(SENTENCES)
		.chars()
		.collect();

	for character in typed.chars() {
		editor.insert_text(character.encode_utf8(&mut [0; 4]))?;
	}

	editor.frame()?;
	assert_eq!(editor.plain_text(), typed);
	assert_eq!(editor.export(DocFormat::Text).trim_end(), typed.trim_end());
	Ok(())
}

#[test]
fn two_editors_converge_under_random_concurrent_input() -> Result<(), CanvistError> {
	let first = editor(1);
	first.share(1)?;
	let second = editor(2);
	second.join(first.encode_state()?, 2)?;
	let mut rng = Rng(CONCURRENT_SEED);

	for round in 0..CONCURRENT_ROUNDS {
		let editor = if round % 2 == 0 { &first } else { &second };
		step(editor, &mut rng)?;
		// A composition stays local until it commits.
		editor.unmark_text()?;

		if rng.below(EXCHANGE_ONE_IN) == 0 {
			exchange(&first, &second)?;
		}

		assert_consistent(&first);
		assert_consistent(&second);
	}

	exchange(&first, &second)?;

	assert_eq!(first.plain_text(), second.plain_text());
	first.frame()?;
	second.frame()?;
	Ok(())
}
