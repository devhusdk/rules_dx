import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const report = fs.readFileSync(
	path.join(
		path.dirname(fileURLToPath(import.meta.url)),
		"check.astro_check.txt",
	),
	"utf8",
);

test("astro check records the wrong-typed prop", () => {
	expect(report).toMatch(/^exit_code: 1\n/);
	expect(report).toContain("src/pages/index.astro:7:15 - error ts(2322)");
	expect(report).toContain("Type 'number' is not assignable to type 'string'.");
	expect(report).toContain("1 error");
});
