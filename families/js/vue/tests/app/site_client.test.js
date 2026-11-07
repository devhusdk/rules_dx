/**
 * @jest-environment jsdom
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const site = path.join(path.dirname(fileURLToPath(import.meta.url)), "site");

test("mounts the built app and handles a click", async () => {
	const html = fs.readFileSync(path.join(site, "index.html"), "utf8");
	const entry = /<script type="module"[^>]*src="\/([^"]+)"/.exec(html)[1];
	document.body.innerHTML = /<body>([\s\S]*)<\/body>/.exec(html)[1];
	await import(pathToFileURL(path.join(site, entry)).href);

	const button = document.querySelector("button.counter");
	expect(button.textContent.trim()).toBe("Count: 2");
	button.click();
	await new Promise((resolve) => setTimeout(resolve, 0));
	expect(button.textContent.trim()).toBe("Count: 3");
	expect(document.querySelector(".total").textContent).toBe("Last change: 3");
});
