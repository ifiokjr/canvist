/**
 * Comparing generated extension descriptors with the metadata Rust reports.
 *
 * Descriptions are comments in the generated file, not data, so both sides
 * are compared without their `doc` fields, sorted by extension name.
 *
 * @module
 */

/** `value` as plain JSON with every `doc` field dropped, sorted by name. */
export function comparable(value: unknown): { name: string }[] {
	const plain: { name: string }[] = JSON.parse(
		JSON.stringify(value, (key, item) => key === "doc" ? undefined : item),
	);

	return plain.sort((a, b) => a.name.localeCompare(b.name));
}
