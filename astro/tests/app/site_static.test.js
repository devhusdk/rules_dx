import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const site = path.join(path.dirname(fileURLToPath(import.meta.url)), "site");
const html = fs.readFileSync(path.join(site, "index.html"), "utf8");

describe("astro_build site", () => {
	test("renders the static components", () => {
		expect(html).toMatch(/<h1[^>]*>Hello from Astro<\/h1>/);
		expect(html).toMatch(/<h2>Counter<\/h2>/);
		expect(html).toMatch(/\.site-header\[data-astro-cid-[a-z0-9]+\]/);
	});

	test("emits a hashed image that exists in the tree", () => {
		const match = /src="\/(_astro\/logo\.[\w-]+\.png)"/.exec(html);
		expect(match).not.toBeNull();
		expect(fs.existsSync(path.join(site, match[1]))).toBe(true);
	});

	test("emits a client:load island with its chunks", () => {
		const island = /<astro-island [^>]*>/.exec(html);
		expect(island).not.toBeNull();
		expect(island[0]).toContain('client="load"');
		for (const attr of ["component-url", "renderer-url"]) {
			const url = new RegExp(`${attr}="/([^"]+)"`).exec(island[0]);
			expect(url).not.toBeNull();
			expect(fs.existsSync(path.join(site, url[1]))).toBe(true);
		}
	});

	test("copies public files", () => {
		expect(fs.readFileSync(path.join(site, "robots.txt"), "utf8")).toContain(
			"User-agent: *",
		);
	});
});
