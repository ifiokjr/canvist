/**
 * The seven hosts and where each one stands, from docs/src/quick-start/readme.md.
 * Keep the status lines honest: say live only for hosts whose tests run on
 * every change.
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
		statusText: "available · React, Vue, SSR",
	},
	{
		id: "desktop",
		name: "Desktop",
		how: "a winit window painted with vello",
		status: "live",
		statusText: "available · macOS, Windows, Linux",
	},
	{
		id: "ios",
		name: "iOS",
		how: "CanvistView over UITextInput and CoreText",
		status: "live",
		statusText: "available · UIKit, SwiftUI",
	},
	{
		id: "android",
		name: "Android",
		how: "CanvistView over InputConnection",
		status: "live",
		statusText: "available · Views, Jetpack Compose",
	},
	{
		id: "flutter",
		name: "Flutter",
		how: "a plugin over flutter_rust_bridge",
		status: "live",
		statusText: "available · iOS, Android, desktop",
	},
	{
		id: "react-native",
		name: "React Native",
		how: "a Fabric component over the iOS and Android views",
		status: "live",
		statusText: "available · iOS, Android",
	},
	{
		id: "lynx",
		name: "Lynx",
		how: "a custom native element over the iOS and Android views",
		status: "live",
		statusText: "available · iOS, Android",
	},
];
