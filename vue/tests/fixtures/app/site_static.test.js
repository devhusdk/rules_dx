import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const site = path.join(path.dirname(fileURLToPath(import.meta.url)), "site");
const html = fs.readFileSync(path.join(site, "index.html"), "utf8");

describe("vue_build site", () => {
	test("links a hashed entry chunk that exists in the tree", () => {
		const match = /<script type="module"[^>]*src="\/(assets\/[\w-]+\.js)"/.exec(
			html,
		);
		expect(match).not.toBeNull();
		expect(fs.existsSync(path.join(site, match[1]))).toBe(true);
	});

	test("emits the hashed logo", () => {
		const assets = fs.readdirSync(path.join(site, "assets"));
		expect(assets.some((name) => /^logo-[\w-]+\.png$/.test(name))).toBe(true);
	});

	test("copies public files", () => {
		expect(fs.readFileSync(path.join(site, "robots.txt"), "utf8")).toContain(
			"User-agent: *",
		);
	});
});
