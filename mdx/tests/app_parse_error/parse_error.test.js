import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const report = fs
	.readFileSync(
		path.join(
			path.dirname(fileURLToPath(import.meta.url)),
			"check.mdx_check.txt",
		),
		"utf8",
	)
	.split("\n");

test("mdx_check names each broken file at its line and column", () => {
	expect(report[0]).toBe("exit_code: 1");
	expect(report).toContain(
		"mdx/tests/app_parse_error/src/BadEsm.mdx:1:18: Could not parse import/exports with acorn [micromark-extension-mdxjs-esm:acorn]",
	);
	expect(report).toContain(
		"mdx/tests/app_parse_error/src/Broken.mdx:5:1: Expected a closing tag for `<div>` (5:1-5:6) [mdast-util-mdx-jsx:end-tag-mismatch]",
	);
});
