/**
 * Which modifier `Mod` means in key bindings.
 *
 * @module
 */

/** Which modifier `Mod` means in key bindings: Cmd on Apple platforms, Ctrl elsewhere. */
export type KeyPlatform = "apple" | "other";

/** Whether `navigator` belongs to an Apple platform, where `Mod` is Cmd. */
export function detectPlatform(navigator: Navigator): KeyPlatform {
	const data = (navigator as Navigator & {
		readonly userAgentData?: { readonly platform: string };
	}).userAgentData;

	return /Mac|iPhone|iPad|iPod/i.test(data?.platform ?? navigator.platform)
		? "apple"
		: "other";
}
