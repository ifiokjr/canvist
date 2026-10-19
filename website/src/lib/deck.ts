/**
 * The landing page's fan deck: one swatch chip per section, in scroll order.
 * `chip` picks the tint from the ramp in tokens.css; `ink` is the label
 * colour that reads on that tint in both themes.
 */
export interface DeckChip {
	id: string;
	number: string;
	name: string;
	chip: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7;
	ink: string;
}

const DARK_ON_LIGHT = "var(--chip-ink)";
const LIGHT_ON_PAINT = "var(--paper)";
const FLIPS = "light-dark(var(--paper), var(--ink))";

export const deck: DeckChip[] = [
	{ id: "top", number: "01", name: "Wet Caret", chip: 0, ink: DARK_ON_LIGHT },
	{
		id: "every-pixel",
		number: "02",
		name: "Owns every pixel",
		chip: 1,
		ink: DARK_ON_LIGHT,
	},
	{
		id: "everywhere",
		number: "03",
		name: "Everywhere",
		chip: 2,
		ink: DARK_ON_LIGHT,
	},
	{
		id: "together",
		number: "04",
		name: "Together",
		chip: 3,
		ink: DARK_ON_LIGHT,
	},
	{
		id: "typed",
		number: "05",
		name: "Headless and typed",
		chip: 4,
		ink: LIGHT_ON_PAINT,
	},
	{ id: "speed", number: "06", name: "Measured speed", chip: 5, ink: FLIPS },
	{ id: "free", number: "07", name: "Free forever", chip: 6, ink: FLIPS },
	{ id: "services", number: "08", name: "Services", chip: 7, ink: FLIPS },
	{
		id: "start",
		number: "09",
		name: "Start painting",
		chip: 4,
		ink: LIGHT_ON_PAINT,
	},
];

export function chipFor(id: string): DeckChip {
	const chip = deck.find((entry) => entry.id === id);
	if (!chip) throw new Error(`No deck chip with id "${id}"`);
	return chip;
}
