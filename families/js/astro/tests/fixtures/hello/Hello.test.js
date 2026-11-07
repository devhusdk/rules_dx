import { parse } from "@astrojs/compiler/sync";
import { is } from "@astrojs/compiler/utils";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const source = fs.readFileSync(path.join(here, "Hello.astro"), "utf8");
// `@astrojs/compiler/sync` exposes the documented synchronous `parse`
// over the pinned Go+WASM compiler; the async entrypoint is not
// asserted here.
const result = parse(source);

describe("Hello.astro", () => {
	test("parses with frontmatter, template, style, and client script regions", () => {
		expect(result.ast.type).toBe("root");
		const frontmatter = result.ast.children.find((node) =>
			is.frontmatter(node),
		);
		expect(frontmatter).toBeDefined();
		expect(frontmatter.value).toMatch("./helper.js");
		// Regions are read off the root's direct children with the
		// authoritative `is` guards. (`walk` is deliberately not used: it
		// fires the visitor asynchronously without returning a promise, so a
		// synchronous test cannot observe its visits deterministically.)
		const elements = result.ast.children
			.filter((node) => is.element(node))
			.map((node) => node.name);
		expect(elements).toEqual(
			expect.arrayContaining(["div", "style", "script"]),
		);
	});

	test("reports no diagnostics", () => {
		expect(result.diagnostics).toEqual([]);
	});
});
