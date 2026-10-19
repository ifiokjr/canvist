import {
	prefersReducedMotion,
	resolveColor,
	trackVisibility,
} from "./motion.ts";

/**
 * "Together": a simulated room where four carets converge on one sentence
 * and type into it at once. Each insertion maps every other caret through
 * it, the way canvist maps selections through remote steps, so carets after
 * an insertion shift right and nobody's cursor jumps.
 *
 * Painted on a canvas, like the editor itself. The carets fly in once when
 * the room first comes into view, then type on a clock; the loop runs only
 * while the room is visible and repaints only when something changed.
 */

interface Actor {
	name: string;
	color: string;
	/** Where in BASE this actor's edit lands; ties resolve in actor order. */
	anchor: number;
	insert: string;
	/** Typing starts this long after the carets have met. */
	delayMs: number;
	msPerChar: number;
	/** Where the caret starts, as a fraction of the canvas. */
	from: { x: number; y: number };
}

const BASE = "Meet at the canvas.";
const FINAL = "Meet me at the big blue canvas tonight.";

const ACTORS: Actor[] = [
	{
		name: "You",
		color: "#3B2CF5",
		anchor: 5,
		insert: "me ",
		delayMs: 200,
		msPerChar: 130,
		from: { x: 0.12, y: 0.86 },
	},
	{
		name: "Ada",
		color: "#E5484D",
		anchor: 12,
		insert: "big ",
		delayMs: 500,
		msPerChar: 150,
		from: { x: 0.78, y: 0.16 },
	},
	{
		name: "Mei",
		color: "#0E9384",
		anchor: 12,
		insert: "blue ",
		delayMs: 900,
		msPerChar: 120,
		from: { x: 0.9, y: 0.78 },
	},
	{
		name: "Kofi",
		color: "#C07A00",
		anchor: 18,
		insert: " tonight",
		delayMs: 1400,
		msPerChar: 110,
		from: { x: 0.3, y: 0.08 },
	},
];

const FILLER_BEFORE = [
	"Saturday at the studio",
	"Bring brushes, rollers and the big sheets.",
];
const FILLER_AFTER = ["Nobody leaves until the drips are dry."];
const HOLD_MS = 2600;
const APPROACH_MS = 1100;

interface Caret {
	actor: Actor;
	offset: number;
	typed: number;
}

interface Room {
	text: string;
	carets: Caret[];
}

function freshRoom(): Room {
	return {
		text: BASE,
		carets: ACTORS.map((actor) => ({ actor, offset: actor.anchor, typed: 0 })),
	};
}

/** Inserts one character for `caret` and maps every other caret through it. */
function typeOne(room: Room, caret: Caret): void {
	const char = caret.actor.insert[caret.typed];
	if (char === undefined) return;
	const at = caret.offset;
	room.text = room.text.slice(0, at) + char + room.text.slice(at);
	const order = room.carets.indexOf(caret);
	for (const [index, other] of room.carets.entries()) {
		if (other === caret) continue;
		// Carets after the insertion point move right. At the same point, the
		// later actor in the tie order moves too, so concurrent text keeps a
		// stable order instead of interleaving letter by letter.
		if (other.offset > at || (other.offset === at && index > order)) {
			other.offset += 1;
		}
	}
	caret.offset += 1;
	caret.typed += 1;
}

function finishedRoom(): Room {
	const room = freshRoom();
	let progressed = true;
	while (progressed) {
		progressed = false;
		for (const caret of room.carets) {
			if (caret.typed < caret.actor.insert.length) {
				typeOne(room, caret);
				progressed = true;
			}
		}
	}
	return room;
}

if (finishedRoom().text !== FINAL) {
	throw new Error(
		`Together demo converges to "${finishedRoom().text}", expected "${FINAL}"`,
	);
}

const ease = (t: number): number => 1 - (1 - t) ** 3;

export function mountTogether(root: HTMLElement): void {
	const canvas = root.querySelector<HTMLCanvasElement>(
		"[data-together-canvas]",
	);
	const log = root.querySelector<HTMLElement>("[data-together-log]");
	const context = canvas?.getContext("2d");
	if (!canvas || !log || !context) {
		throw new Error("Together section markup is incomplete");
	}
	const ctx = context;

	let width = 0;
	let height = 0;
	let room = prefersReducedMotion() ? finishedRoom() : freshRoom();
	let approach = prefersReducedMotion() ? 1 : 0;
	let approachStartedAt: number | undefined;
	let painted = "";
	let typingStartedAt: number | undefined;
	let visible = false;
	let frame = 0;
	let lastLog = "";

	function resize(): void {
		if (!canvas) return;
		const rect = canvas.getBoundingClientRect();
		const dpr = Math.min(devicePixelRatio, 2);
		width = rect.width;
		height = rect.height;
		canvas.width = Math.round(width * dpr);
		canvas.height = Math.round(height * dpr);
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		draw();
	}

	/** The largest size at which every line, title included, fits the canvas. */
	function fontSize(): number {
		ctx.font = "560 100px Bricolage, system-ui, sans-serif";
		const widest = Math.max(
			...[FINAL, ...FILLER_AFTER, ...FILLER_BEFORE.slice(1)].map((line) =>
				ctx.measureText(line).width
			),
			ctx.measureText(FILLER_BEFORE[0] ?? "").width * 1.25,
		);
		const fit = ((width - 2 * pad()) / widest) * 100;
		return Math.max(10, Math.min(26, fit));
	}

	function pad(): number {
		return Math.max(18, width * 0.06);
	}

	function draw(): void {
		if (!canvas || width === 0) return;
		const ink = resolveColor(canvas.parentElement ?? canvas, "var(--text)");
		const muted = resolveColor(
			canvas.parentElement ?? canvas,
			"var(--text-muted)",
		);
		const size = fontSize();
		const lineHeight = size * 1.9;
		const left = pad();
		const lines = [...FILLER_BEFORE, "", ...FILLER_AFTER];
		const sharedIndex = FILLER_BEFORE.length;
		const top = (height - lines.length * lineHeight) / 2 + size;

		ctx.clearRect(0, 0, width, height);
		ctx.textBaseline = "alphabetic";

		for (const [index, line] of lines.entries()) {
			if (index === sharedIndex) continue;
			const isTitle = index === 0;
			ctx.font = `${isTitle ? 750 : 450} ${
				isTitle ? size * 1.25 : size
			}px Bricolage, system-ui, sans-serif`;
			ctx.fillStyle = isTitle ? ink : muted;
			ctx.fillText(line, left, top + index * lineHeight);
		}

		const baseline = top + sharedIndex * lineHeight;
		ctx.font = `560 ${size}px Bricolage, system-ui, sans-serif`;
		ctx.fillStyle = ink;
		ctx.fillText(room.text, left, baseline);

		// Carets fly in from their starting spots to their place in the line.
		const t = ease(Math.min(1, Math.max(0, approach)));
		for (const caret of room.carets) {
			const targetX = left +
				ctx.measureText(room.text.slice(0, caret.offset)).width;
			const startX = caret.actor.from.x * width;
			const startY = caret.actor.from.y * height;
			const x = startX + (targetX - startX) * t;
			const y = startY + (baseline - startY) * t;
			drawCaret(caret.actor, x, y, size);
		}
	}

	function drawCaret(
		actor: Actor,
		x: number,
		baseline: number,
		size: number,
	): void {
		const caretTop = baseline - size * 0.95;
		const caretHeight = size * 1.25;
		ctx.fillStyle = actor.color;
		ctx.fillRect(Math.round(x) - 1, caretTop, 2, caretHeight);
		// Name flag.
		ctx.font = `600 ${
			Math.max(10, size * 0.52)
		}px Bricolage, system-ui, sans-serif`;
		const label = actor.name;
		const labelWidth = ctx.measureText(label).width + 10;
		const flagHeight = Math.max(14, size * 0.78);
		const flagX = Math.min(Math.round(x) - 1, width - labelWidth - 2);
		ctx.beginPath();
		ctx.roundRect(flagX, caretTop - flagHeight, labelWidth, flagHeight, [
			4,
			4,
			4,
			0,
		]);
		ctx.fill();
		ctx.fillStyle = "#FFFFFF";
		ctx.fillText(label, flagX + 5, caretTop - flagHeight * 0.28);
	}

	function writeLog(text: string): void {
		if (!log || text === lastLog) return;
		lastLog = text;
		log.textContent = text;
	}

	function tick(now: number): void {
		frame = 0;
		approachStartedAt ??= now;
		approach = Math.min(1, (now - approachStartedAt) / APPROACH_MS);
		if (approach >= 1) {
			typingStartedAt ??= now;
			const elapsed = now - typingStartedAt;
			const typing: string[] = [];
			for (const caret of room.carets) {
				const due = Math.floor(
					(elapsed - caret.actor.delayMs) / caret.actor.msPerChar,
				);
				while (caret.typed < Math.min(due, caret.actor.insert.length)) {
					typeOne(room, caret);
				}
				if (due >= 0 && caret.typed < caret.actor.insert.length) {
					typing.push(caret.actor.name);
				}
			}
			const done = room.carets.every((caret) =>
				caret.typed === caret.actor.insert.length
			);
			writeLog(
				typing.length > 0
					? `${typing.join(", ")} typing…`
					: done
					? "All four edits merged. Same text for everyone."
					: "4 people in this document",
			);
			const finishedAt = Math.max(
				...ACTORS.map((actor) =>
					actor.delayMs + actor.msPerChar * actor.insert.length
				),
			);
			if (elapsed > finishedAt + HOLD_MS) {
				room = freshRoom();
				typingStartedAt = undefined;
			}
		} else {
			writeLog(
				approach > 0 ? "Ada, Mei and Kofi joined" : "4 people in this document",
			);
		}
		// Repaint only when the text or the carets moved.
		const state = `${approach}|${room.text}|${
			room.carets.map((caret) => caret.offset).join(",")
		}`;
		if (state !== painted) {
			painted = state;
			draw();
		}
		if (visible) frame = requestAnimationFrame(tick);
	}

	function schedule(): void {
		if (frame === 0) frame = requestAnimationFrame(tick);
	}

	new ResizeObserver(resize).observe(canvas);
	document.addEventListener("canvist:theme", () => requestAnimationFrame(draw));
	void document.fonts.load("560 20px Bricolage").then(draw);

	if (prefersReducedMotion()) {
		writeLog("All four edits merged. Same text for everyone.");
		return;
	}

	trackVisibility(canvas, (isVisible) => {
		visible = isVisible;
		if (visible) schedule();
	});
}
