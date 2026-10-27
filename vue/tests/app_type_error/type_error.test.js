import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const report = fs.readFileSync(
	path.join(
		path.dirname(fileURLToPath(import.meta.url)),
		"check.vue_check.txt",
	),
	"utf8",
);

test("vue-tsc records the wrong-typed prop", () => {
	expect(report).toMatch(/^exit_code: 2\n/);
	expect(report).toContain("src/App.vue(6,13): error TS2322");
	expect(report).toContain("Type 'string' is not assignable to type 'number'.");
});
