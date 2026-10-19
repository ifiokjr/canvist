import { assert, assertAlmostEquals, assertEquals } from "@std/assert";
import {
	findBudget,
	firstDuration,
	loadBudgets,
	parseBudgetTable,
	parseDuration,
} from "../../src/lib/budgets.ts";

const ARCHITECTURE =
	new URL("../../../docs/src/guides/performance.md", import.meta.url).pathname;

Deno.test("parseDuration reads the units the budget table uses", () => {
	assertEquals(parseDuration("0.29 µs")?.seconds, 0.29e-6);
	assertEquals(parseDuration("< 2 ms")?.seconds, 2e-3);
	assertEquals(parseDuration("24 ms")?.text, "24 ms");
	assertEquals(parseDuration("83 µs / 1.4 ms"), undefined);
	assertEquals(parseDuration("(M3)"), undefined);
});

Deno.test("firstDuration reads paired cells, borrowing a unit written once at the end", () => {
	assertEquals(firstDuration("83 µs / 1.4 ms")?.text, "83 µs");
	assertAlmostEquals(firstDuration("< 200 µs / < 2 ms")?.seconds ?? 0, 200e-6);
	assertEquals(firstDuration("19 / 7 / 2.4 ms")?.text, "19 ms");
	assertEquals(firstDuration("1,058 KB (433 KB gz)"), undefined);
	assertEquals(firstDuration("layout 0.6 ms (M2)"), undefined);
});

Deno.test("parseBudgetTable fails loudly when the section is missing", () => {
	let message = "";
	try {
		parseBudgetTable("# Nothing here");
	} catch (error) {
		message = error instanceof Error ? error.message : String(error);
	}
	assert(message.includes("Performance budgets"));
});

Deno.test("the performance page's budgets parse, and every measurement is within budget", () => {
	const table = loadBudgets(ARCHITECTURE);
	assert(
		table.measured.length >= 10,
		`expected at least 10 measured rows, got ${table.measured.length}`,
	);
	for (const row of table.measured) {
		assert(
			row.today.seconds <= row.budget.seconds,
			`${row.metric}: ${row.today.text} is over ${row.budgetText}`,
		);
	}
	// The landing page headline depends on this row.
	assertEquals(findBudget(table, "Apply one keystroke").today.text, "0.29 µs");
	// Measured by @canvist/web's browser tests: input to painted canvas.
	assertEquals(findBudget(table, "Keystroke → paint").today.text, "0.5 ms");
});
