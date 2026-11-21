/**
 * @jest-environment jsdom
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const site = path.join(path.dirname(fileURLToPath(import.meta.url)), "site");

test("mounts the built app and handles a click", async () => {
	const page = new DOMParser().parseFromString(
		fs.readFileSync(path.join(site, "index.html"), "utf8"),
		"text/html",
	);
	const entry = page
		.querySelector('script[type="module"]')
		.getAttribute("src")
		.slice(1);
	document.body.innerHTML = page.body.innerHTML;
	await import(pathToFileURL(path.join(site, entry)).href);

	expect(document.querySelectorAll(".card")).toHaveLength(2);
	const button = document.querySelector("button.counter");
	expect(button.textContent).toBe("Count: 2");
	button.click();
	await new Promise((resolve) => setTimeout(resolve, 0));
	expect(button.textContent).toBe("Count: 3");
});
