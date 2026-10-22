/**
 * A notes screen on `@canvist/lynx`: the native editor, a toolbar that
 * follows the selection, the note as Markdown, and a benchmark of the
 * JavaScript-to-native calls. The end-to-end flows in `e2e/` drive it.
 */
import {
	CanvistView,
	type Editor,
	type StarterKit,
	useCanvistEditor,
	useEditorState,
} from "@canvist/lynx";
import { useInitData, useState } from "@lynx-js/react";

import "./App.css";
import { benchmark, type BenchmarkResult } from "./benchmark.ts";

const NOTE = `# Field notes

Tap anywhere to write. Type **##** and a space for a heading, **-** for a list, or **[ ]** for a checklist.

- [ ] Try the toolbar
- [ ] Undo with a shake`;

declare module "@lynx-js/react" {
	interface InitData {
		/** Multiplies the benchmark's budgets; CI's end-to-end flows pass it. */
		readonly perfSlack?: number;
	}
}

type NoteEditor = Editor<StarterKit>;
type Panel = "none" | "markdown" | "benchmark";

export function App() {
	const { editor, props } = useCanvistEditor({
		content: { format: "markdown", input: NOTE },
	});
	const [panel, setPanel] = useState<Panel>("none");
	const { perfSlack = 1 } = useInitData();
	const toggle = (next: Panel) => setPanel(panel === next ? "none" : next);

	return (
		<view className="screen">
			<Toolbar editor={editor} />
			<CanvistView {...props} className="editor" accessibility-label="Note" />
			{panel === "markdown" && <Markdown editor={editor} />}
			{panel === "benchmark" && <Benchmark editor={editor} slack={perfSlack} />}
			<view className="footer" ignore-focus>
				<Button
					label="Markdown"
					active={panel === "markdown"}
					onTap={() => toggle("markdown")}
				/>
				<Button
					label="Benchmark"
					active={panel === "benchmark"}
					onTap={() => toggle("benchmark")}
				/>
				<WordCount editor={editor} />
			</view>
		</view>
	);
}

/** The formatting toolbar: each button lights up while its format applies at the selection. */
function Toolbar({ editor }: { readonly editor: NoteEditor | null }) {
	const bold = useEditorState(editor, (e) => e.query.isBold(), false);
	const italic = useEditorState(editor, (e) => e.query.isItalic(), false);
	const heading = useEditorState(editor, (e) => e.query.headingLevel(), null);
	const bullets = useEditorState(editor, (e) => e.query.isBulletList(), false);
	const tasks = useEditorState(editor, (e) => e.query.isTaskList(), false);
	const canUndo = useEditorState(editor, (e) => e.canUndo(), false);
	const canRedo = useEditorState(editor, (e) => e.canRedo(), false);

	return (
		// `ignore-focus`: tapping a tool keeps the editor's keyboard up.
		<view className="toolbar" ignore-focus>
			<Tool
				symbol="B"
				label="Bold"
				active={bold}
				onTap={() => editor?.commands.toggleBold()}
			/>
			<Tool
				symbol="I"
				label="Italic"
				active={italic}
				onTap={() => editor?.commands.toggleItalic()}
			/>
			<Tool
				symbol="H2"
				label="Heading"
				active={heading === 2}
				onTap={() => editor?.commands.toggleHeading({ level: 2 })}
			/>
			<Tool
				symbol="•"
				label="Bulleted list"
				active={bullets}
				onTap={() => editor?.commands.toggleBulletList()}
			/>
			<Tool
				symbol="☐"
				label="Checklist"
				active={tasks}
				onTap={() => editor?.commands.toggleTaskList()}
			/>
			<view className="divider" />
			<Tool
				symbol="↶"
				label="Undo"
				disabled={!canUndo}
				onTap={() => editor?.undo()}
			/>
			<Tool
				symbol="↷"
				label="Redo"
				disabled={!canRedo}
				onTap={() => editor?.redo()}
			/>
		</view>
	);
}

function Tool(
	{ symbol, label, active = false, disabled = false, onTap }: {
		readonly symbol: string;
		readonly label: string;
		readonly active?: boolean;
		readonly disabled?: boolean;
		readonly onTap: () => void;
	},
) {
	return (
		<view
			className={`tool${active ? " tool--active" : ""}${
				disabled ? " tool--disabled" : ""
			}`}
			accessibility-element
			flatten={false}
			accessibility-label={label}
			accessibility-traits="button"
			bindtap={disabled ? undefined : onTap}
		>
			<text className="tool__symbol">{symbol}</text>
		</view>
	);
}

function Button(
	{ label, active, onTap }: {
		readonly label: string;
		readonly active: boolean;
		readonly onTap: () => void;
	},
) {
	return (
		<view
			className="footer__button"
			accessibility-element
			flatten={false}
			accessibility-label={label}
			accessibility-traits="button"
			bindtap={onTap}
		>
			<text
				className={`footer__label${active ? " footer__label--active" : ""}`}
			>
				{label}
			</text>
		</view>
	);
}

function WordCount({ editor }: { readonly editor: NoteEditor | null }) {
	const words = useEditorState(
		editor,
		(e) => e.export("text").split(/\s+/).filter(Boolean).length,
		0,
	);
	const label = words === 1 ? "1 word" : `${words} words`;

	return (
		<text
			className="word-count"
			accessibility-element
			flatten={false}
			accessibility-label={label}
		>
			{label}
		</text>
	);
}

/** The note as Markdown, kept up to date as you type. */
function Markdown({ editor }: { readonly editor: NoteEditor | null }) {
	const markdown = useEditorState(editor, (e) => e.export("markdown"), "");

	return (
		<scroll-view className="panel" scroll-orientation="vertical">
			<text
				className="mono"
				accessibility-element
				flatten={false}
				accessibility-label={markdown}
			>
				{markdown}
			</text>
		</scroll-view>
	);
}

/** Time the synchronous calls into the native editor and check them against their budgets. */
function Benchmark(
	{ editor, slack }: {
		readonly editor: NoteEditor | null;
		readonly slack: number;
	},
) {
	const [result, setResult] = useState<BenchmarkResult | null>(null);
	const [failure, setFailure] = useState<string | null>(null);

	return (
		<view className="panel">
			<view
				className="run"
				accessibility-element
				flatten={false}
				accessibility-label="Run benchmark"
				accessibility-traits="button"
				bindtap={() => {
					if (editor === null) return;

					try {
						setResult(benchmark(editor, slack));
						setFailure(null);
					} catch (error) {
						setFailure(
							`Benchmark failed: ${
								error instanceof Error ? error.message : String(error)
							}`,
						);
					}
				}}
			>
				<text className="run__label">Run benchmark</text>
			</view>
			{failure !== null && (
				<text
					className="mono"
					accessibility-element
					flatten={false}
					accessibility-label={failure}
				>
					{failure}
				</text>
			)}
			{result !== null && (
				<view>
					<text className="mono mono--muted">call p50 p95 budget (µs)</text>
					{result.rows.map((row) => (
						<text key={row.name} className="mono">
							{[
								row.name.padEnd(10),
								row.p50.toFixed(0).padStart(5),
								row.p95.toFixed(0).padStart(6),
								String(row.budget).padStart(7),
							].join(" ")}
						</text>
					))}
					<text
						className="verdict"
						accessibility-element
						flatten={false}
						accessibility-label={result.withinBudget
							? "Within budget"
							: "Over budget"}
					>
						{result.withinBudget ? "Within budget" : "Over budget"}
					</text>
				</view>
			)}
		</view>
	);
}
