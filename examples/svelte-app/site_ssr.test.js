import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ssr = path.join(path.dirname(fileURLToPath(import.meta.url)), "site_ssr");

test("renders the app on the server", async () => {
	const entry = fs.readdirSync(ssr).find((name) => /\.m?js$/.test(name));
	const { render } = await import(pathToFileURL(path.join(ssr, entry)).href);
	const html = render();
	expect(html).toContain("<h1>Hello from Svelte</h1>");
	expect(html).toMatch(/<img src="\/assets\/logo-[\w-]+\.png" alt="logo"/);
	expect(html).toMatch(/Count: 2/);
	expect(html).toContain("Last change: 0");
});
