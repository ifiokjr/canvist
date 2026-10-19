/**
 * The seven hosts and where each one stands, from docs/src/quick-start/readme.md.
 * Keep the status lines honest: only the v1 web editor runs today.
 */
export type PlatformId =
	| "web"
	| "desktop"
	| "ios"
	| "android"
	| "flutter"
	| "react-native"
	| "lynx";

export interface Platform {
	id: PlatformId;
	name: string;
	/** How the host draws, in one line. */
	how: string;
	status: "live" | "building" | "planned";
	statusText: string;
}

export const platforms: Platform[] = [
	{
		id: "web",
		name: "Web",
		how: "wasm and Canvas2D, input through EditContext",
		status: "live",
		statusText: "v1 live · v2 host in M3",
	},
	{
		id: "desktop",
		name: "Desktop",
		how: "a winit window painted with vello",
		status: "planned",
		statusText: "M5",
	},
	{
		id: "ios",
		name: "iOS",
		how: "CanvistView over UITextInput and CoreText",
		status: "building",
		statusText: "M6 · toolchain ready",
	},
	{
		id: "android",
		name: "Android",
		how: "CanvistView over InputConnection",
		status: "building",
		statusText: "M6 · toolchain ready",
	},
	{
		id: "flutter",
		name: "Flutter",
		how: "a plugin over flutter_rust_bridge",
		status: "building",
		statusText: "M7 · bridge ready",
	},
	{
		id: "react-native",
		name: "React Native",
		how: "a native component via uniffi-bindgen-react-native",
		status: "planned",
		statusText: "M7",
	},
	{
		id: "lynx",
		name: "Lynx",
		how: "a custom native element over the iOS and Android views",
		status: "planned",
		statusText: "M7",
	},
];
