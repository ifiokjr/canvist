import { assert, assertEquals } from "@std/assert";
import {
	findBudget,
	loadBudgets,
	parseBudgetTable,
	parseDuration,
} from "../../src/lib/budgets.ts";

const ARCHITECTURE =
	new URL("../../../docs/src/v2/architecture.md", import.meta.url).pathname;

Deno.test("parseDuration reads the units the budget table uses", () => {
	assertEquals(parseDuration("0.29 µs")?.seconds, 0.29e-6);
	assertEquals(parseDuration("< 2 ms")?.seconds, 2e-3);
	assertEquals(parseDuration("24 ms")?.text, "24 ms");
	assertEquals(parseDuration("83 µs / 1.4 ms"), undefined);
	assertEquals(parseDuration("(M3)"), undefined);
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

Deno.test("the architecture doc's budgets parse, and every measurement is within budget", () => {
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
	assert(
		table.pending.some((row) => row.metric.startsWith("Keystroke → paint")),
	);
});
