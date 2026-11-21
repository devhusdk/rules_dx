/**
 * @jest-environment jsdom
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const site = path.join(path.dirname(fileURLToPath(import.meta.url)), "site");
const page = new DOMParser().parseFromString(
	fs.readFileSync(path.join(site, "index.html"), "utf8"),
	"text/html",
);

describe("mdx_build site", () => {
	test("links an entry chunk that exists in the tree", () => {
		const script = page.querySelector('script[type="module"]');
		const src = script.getAttribute("src");
		expect(src).toMatch(/^\/assets\/[\w-]+\.js$/);
		expect(fs.existsSync(path.join(site, src))).toBe(true);
	});

	test("emits the hashed logo", () => {
		const assets = fs.readdirSync(path.join(site, "assets"));
		expect(assets.some((name) => /^logo-[\w-]+\.png$/.test(name))).toBe(true);
	});
});
