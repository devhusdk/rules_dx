import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const site = path.join(path.dirname(fileURLToPath(import.meta.url)), "site");
const html = fs.readFileSync(path.join(site, "index.html"), "utf8");

describe("astro_build config", () => {
	test("uses the configured base", () => {
		expect(html).toContain('<p id="base">/docs</p>');
		expect(html).not.toContain("/wrong");
	});

	test("uses the configured site", () => {
		expect(html).toMatch(/<p id="site">https:\/\/example\.test\/?<\/p>/);
	});
});
