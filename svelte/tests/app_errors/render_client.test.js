/**
 * @jest-environment jsdom
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const site = path.join(
	path.dirname(fileURLToPath(import.meta.url)),
	"throwing/site",
);

test("mounting the built app rejects with the thrown error", async () => {
	const html = fs.readFileSync(path.join(site, "index.html"), "utf8");
	const entry = /<script type="module"[^>]*src="\/([^"]+)"/.exec(html)[1];
	document.body.innerHTML = '<div id="app"></div>';
	await expect(
		import(pathToFileURL(path.join(site, entry)).href),
	).rejects.toThrow("render boom");
	expect(document.getElementById("app").textContent).toBe("");
});
