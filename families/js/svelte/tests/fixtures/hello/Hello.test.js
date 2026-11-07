import { parse } from "svelte/compiler";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const source = fs.readFileSync(path.join(here, "Hello.svelte"), "utf8");
// `modern: true` selects the documented Svelte 5 AST (`fragment`,
// `instance`, `css`); the legacy default shape is not asserted here.
const ast = parse(source, { modern: true });

describe("Hello.svelte", () => {
	test("parses with template/fragment, script/instance, and style/css regions", () => {
		expect(ast.fragment).toBeDefined();
		expect(ast.fragment.nodes.length).toBeGreaterThan(0);
		expect(ast.instance).not.toBeNull();
		expect(ast.css).not.toBeNull();
	});

	test("script block imports the local helper", () => {
		const script = source.slice(ast.instance.start, ast.instance.end);
		expect(script).toMatch("./helper.js");
	});
});
