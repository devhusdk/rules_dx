import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const report = fs.readFileSync(
	path.join(
		path.dirname(fileURLToPath(import.meta.url)),
		"check.svelte_check.txt",
	),
	"utf8",
);

test("svelte-check records the wrong-typed prop at the source path", () => {
	expect(report).toMatch(/^exit_code: 1\n/);
	expect(report).toMatch(
		/^ERROR "src\/App\.svelte" \d+:\d+ ".*not assignable to type 'number'/m,
	);
});

test("svelte-check records the parse error", () => {
	expect(report).toMatch(/^ERROR "src\/Broken\.svelte" \d+:\d+/m);
	expect(report).toMatch(/^COMPLETED \d+ FILES [1-9]\d* ERRORS/m);
});
