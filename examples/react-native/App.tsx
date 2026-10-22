/**
 * A notes screen on `@canvist/react-native`: the native editor, a toolbar
 * that follows the selection, the note as Markdown, and a benchmark of the
 * JavaScript-to-native calls. The end-to-end flows in `e2e/` drive it.
 */
import {
	CanvistView,
	type Editor,
	type StarterKit,
	useCanvistEditor,
	useEditorState,
} from "@canvist/react-native";
import { useState } from "react";
import {
	KeyboardAvoidingView,
	Platform,
	Pressable,
	ScrollView,
	StatusBar,
	type StyleProp,
	StyleSheet,
	Text,
	type TextStyle,
	useColorScheme,
	View,
} from "react-native";
import {
	SafeAreaProvider,
	useSafeAreaInsets,
} from "react-native-safe-area-context";

import { benchmark, type BenchmarkResult } from "./benchmark";

const NOTE = `# Field notes

Tap anywhere to write. Type **##** and a space for a heading, **-** for a list, or **[ ]** for a checklist.

- [ ] Try the toolbar
- [ ] Undo with a shake`;

/** `perfSlack` multiplies the benchmark's budgets; CI's end-to-end flows pass it. */
export function App({ perfSlack = 1 }: { readonly perfSlack?: number }) {
	const dark = useColorScheme() === "dark";

	return (
		<SafeAreaProvider>
			<StatusBar barStyle={dark ? "light-content" : "dark-content"} />
			<Notes dark={dark} perfSlack={perfSlack} />
		</SafeAreaProvider>
	);
}

type Panel = "none" | "markdown" | "benchmark";

function Notes(
	{ dark, perfSlack }: { readonly dark: boolean; readonly perfSlack: number },
) {
	const insets = useSafeAreaInsets();
	const colors = dark ? DARK : LIGHT;
	const { editor, props } = useCanvistEditor({
		content: { format: "markdown", input: NOTE },
	});
	const [panel, setPanel] = useState<Panel>("none");

	return (
		<KeyboardAvoidingView
			behavior="padding"
			style={[styles.screen, {
				backgroundColor: colors.page,
				paddingTop: insets.top,
			}]}
		>
			<Toolbar editor={editor} colors={colors} />
			<CanvistView
				{...props}
				testID="editor"
				accessibilityLabel="Note"
				style={styles.editor}
			/>
			{panel === "markdown" && <Markdown editor={editor} colors={colors} />}
			{panel === "benchmark" && (
				<Benchmark editor={editor} colors={colors} slack={perfSlack} />
			)}
			<View
				style={[styles.footer, {
					borderTopColor: colors.rule,
					paddingBottom: insets.bottom + 8,
				}]}
			>
				<FooterButton
					label="Markdown"
					active={panel === "markdown"}
					colors={colors}
					onPress={() => setPanel(panel === "markdown" ? "none" : "markdown")}
				/>
				<FooterButton
					label="Benchmark"
					active={panel === "benchmark"}
					colors={colors}
					onPress={() => setPanel(panel === "benchmark" ? "none" : "benchmark")}
				/>
				<WordCount editor={editor} colors={colors} />
			</View>
		</KeyboardAvoidingView>
	);
}

type NoteEditor = Editor<StarterKit>;

interface Colors {
	readonly page: string;
	readonly ink: string;
	readonly muted: string;
	readonly rule: string;
	readonly chip: string;
	readonly accent: string;
	readonly onAccent: string;
}

const LIGHT: Colors = {
	page: "#ffffff",
	ink: "#17161c",
	muted: "#6b6878",
	rule: "#e7e5ee",
	chip: "#f3f2f8",
	accent: "#3b2cf5",
	onAccent: "#ffffff",
};

const DARK: Colors = {
	page: "#121117",
	ink: "#f1f0f5",
	muted: "#a19eb0",
	rule: "#2a2833",
	chip: "#1f1d27",
	accent: "#8b80ff",
	onAccent: "#121117",
};

/** The formatting toolbar: each button lights up while its format applies at the selection. */
function Toolbar(
	{ editor, colors }: {
		readonly editor: NoteEditor | null;
		readonly colors: Colors;
	},
) {
	const bold = useEditorState(editor, (e) => e.query.isBold(), false);
	const italic = useEditorState(editor, (e) => e.query.isItalic(), false);
	const heading = useEditorState(editor, (e) => e.query.headingLevel(), null);
	const bullets = useEditorState(editor, (e) => e.query.isBulletList(), false);
	const tasks = useEditorState(editor, (e) => e.query.isTaskList(), false);
	const canUndo = useEditorState(editor, (e) => e.canUndo(), false);
	const canRedo = useEditorState(editor, (e) => e.canRedo(), false);

	return (
		<ScrollView
			horizontal
			showsHorizontalScrollIndicator={false}
			keyboardShouldPersistTaps="always"
			style={[styles.toolbar, { borderBottomColor: colors.rule }]}
			contentContainerStyle={styles.toolbarContent}
		>
			<Tool
				label="B"
				name="Bold"
				active={bold}
				colors={colors}
				onPress={() => editor?.commands.toggleBold()}
				textStyle={styles.bold}
			/>
			<Tool
				label="I"
				name="Italic"
				active={italic}
				colors={colors}
				onPress={() => editor?.commands.toggleItalic()}
				textStyle={styles.italic}
			/>
			<Tool
				label="H2"
				name="Heading"
				active={heading === 2}
				colors={colors}
				onPress={() => editor?.commands.toggleHeading({ level: 2 })}
			/>
			<Tool
				label="•"
				name="Bulleted list"
				active={bullets}
				colors={colors}
				onPress={() => editor?.commands.toggleBulletList()}
			/>
			<Tool
				label="☐"
				name="Checklist"
				active={tasks}
				colors={colors}
				onPress={() => editor?.commands.toggleTaskList()}
			/>
			<View style={[styles.divider, { backgroundColor: colors.rule }]} />
			<Tool
				label="↶"
				name="Undo"
				disabled={!canUndo}
				colors={colors}
				onPress={() => editor?.undo()}
			/>
			<Tool
				label="↷"
				name="Redo"
				disabled={!canRedo}
				colors={colors}
				onPress={() => editor?.redo()}
			/>
		</ScrollView>
	);
}

function Tool(
	{ label, name, active = false, disabled = false, colors, onPress, textStyle }:
		{
			readonly label: string;
			readonly name: string;
			readonly active?: boolean;
			readonly disabled?: boolean;
			readonly colors: Colors;
			readonly onPress: () => void;
			readonly textStyle?: StyleProp<TextStyle>;
		},
) {
	return (
		<Pressable
			testID={`tool-${name}`}
			accessibilityRole="button"
			accessibilityLabel={name}
			accessibilityState={{ selected: active, disabled }}
			disabled={disabled}
			onPress={onPress}
			style={({ pressed }) => [
				styles.tool,
				{ backgroundColor: active ? colors.accent : colors.chip },
				(pressed || disabled) && styles.dimmed,
			]}
		>
			<Text
				style={[
					styles.toolLabel,
					{ color: active ? colors.onAccent : colors.ink },
					textStyle,
				]}
			>
				{label}
			</Text>
		</Pressable>
	);
}

function FooterButton(
	{ label, active, colors, onPress }: {
		readonly label: string;
		readonly active: boolean;
		readonly colors: Colors;
		readonly onPress: () => void;
	},
) {
	return (
		<Pressable
			testID={`footer-${label}`}
			accessibilityRole="button"
			accessibilityState={{ expanded: active }}
			onPress={onPress}
			style={styles.footerButton}
		>
			<Text
				style={[styles.footerLabel, {
					color: active ? colors.accent : colors.muted,
				}]}
			>
				{label}
			</Text>
		</Pressable>
	);
}

function WordCount(
	{ editor, colors }: {
		readonly editor: NoteEditor | null;
		readonly colors: Colors;
	},
) {
	const words = useEditorState(
		editor,
		(e) => e.export("text").split(/\s+/).filter(Boolean).length,
		0,
	);

	return (
		<Text
			testID="word-count"
			style={[styles.wordCount, { color: colors.muted }]}
		>
			{words === 1 ? "1 word" : `${words} words`}
		</Text>
	);
}

/** The note as Markdown, kept up to date as you type. */
function Markdown(
	{ editor, colors }: {
		readonly editor: NoteEditor | null;
		readonly colors: Colors;
	},
) {
	const markdown = useEditorState(editor, (e) => e.export("markdown"), "");

	return (
		<ScrollView style={[styles.panel, { backgroundColor: colors.chip }]}>
			<Text
				testID="markdown"
				selectable
				style={[styles.mono, { color: colors.ink }]}
			>
				{markdown}
			</Text>
		</ScrollView>
	);
}

/** Time the synchronous calls into the native editor and check them against their budgets. */
function Benchmark(
	{ editor, colors, slack }: {
		readonly editor: NoteEditor | null;
		readonly colors: Colors;
		readonly slack: number;
	},
) {
	const [result, setResult] = useState<BenchmarkResult | null>(null);

	return (
		<View style={[styles.panel, { backgroundColor: colors.chip }]}>
			<Pressable
				testID="benchmark-run"
				accessibilityRole="button"
				disabled={editor === null}
				onPress={() => editor && setResult(benchmark(editor, slack))}
				style={[styles.run, { backgroundColor: colors.accent }]}
			>
				<Text style={[styles.toolLabel, { color: colors.onAccent }]}>
					Run benchmark
				</Text>
			</Pressable>
			{result !== null && (
				<View testID="benchmark-result">
					<Text style={[styles.mono, { color: colors.muted }]}>
						{"call         p50    p95  budget (µs)"}
					</Text>
					{result.rows.map((row) => (
						<Text key={row.name} style={[styles.mono, { color: colors.ink }]}>
							{[
								row.name.padEnd(10),
								row.p50.toFixed(0).padStart(5),
								row.p95.toFixed(0).padStart(6),
								String(row.budget).padStart(7),
							].join(" ")}
						</Text>
					))}
					<Text
						testID="benchmark-verdict"
						style={[styles.verdict, { color: colors.accent }]}
					>
						{result.withinBudget ? "Within budget" : "Over budget"}
					</Text>
				</View>
			)}
		</View>
	);
}

const styles = StyleSheet.create({
	screen: { flex: 1 },
	toolbar: { flexGrow: 0, borderBottomWidth: StyleSheet.hairlineWidth },
	toolbarContent: {
		paddingHorizontal: 12,
		paddingVertical: 8,
		gap: 6,
		alignItems: "center",
	},
	tool: {
		minWidth: 44,
		height: 36,
		borderRadius: 18,
		paddingHorizontal: 12,
		alignItems: "center",
		justifyContent: "center",
	},
	toolLabel: { fontSize: 16, fontWeight: "600" },
	bold: { fontWeight: "800" },
	italic: {
		fontStyle: "italic",
		fontFamily: Platform.select({ ios: "Georgia", default: "serif" }),
	},
	dimmed: { opacity: 0.45 },
	divider: { width: StyleSheet.hairlineWidth, height: 24, marginHorizontal: 4 },
	editor: { flex: 1 },
	footer: {
		flexDirection: "row",
		alignItems: "center",
		borderTopWidth: StyleSheet.hairlineWidth,
		paddingHorizontal: 12,
		paddingTop: 8,
		gap: 4,
	},
	footerButton: { paddingHorizontal: 8, paddingVertical: 6 },
	footerLabel: { fontSize: 14, fontWeight: "600" },
	wordCount: {
		marginLeft: "auto",
		fontSize: 13,
		fontVariant: ["tabular-nums"],
	},
	panel: { maxHeight: 220, padding: 12 },
	mono: {
		fontFamily: Platform.select({ ios: "Menlo", default: "monospace" }),
		fontSize: 12,
		lineHeight: 18,
	},
	run: {
		alignSelf: "flex-start",
		borderRadius: 18,
		paddingHorizontal: 16,
		height: 36,
		justifyContent: "center",
		marginBottom: 8,
	},
	verdict: { marginTop: 6, fontWeight: "700" },
});
