/**
 * @jest-environment jsdom
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const site = path.join(path.dirname(fileURLToPath(import.meta.url)), "site");

function chunk(island, attr) {
	return pathToFileURL(path.join(site, island.getAttribute(attr))).href;
}

function decodeProps(raw) {
	return Object.fromEntries(
		Object.entries(JSON.parse(raw)).map(([key, [, value]]) => [key, value]),
	);
}

test("hydrates the Counter island from the built chunks", async () => {
	const html = fs.readFileSync(path.join(site, "index.html"), "utf8");
	const body = /<body>([\s\S]*)<\/body>/.exec(html)[1];
	document.body.innerHTML = body.replace(/<script[\s\S]*?<\/script>/g, "");
	const island = document.querySelector("astro-island");
	expect(island.getAttribute("client")).toBe("load");

	const component = await import(chunk(island, "component-url"));
	const renderer = await import(chunk(island, "renderer-url"));
	const props = decodeProps(island.getAttribute("props"));
	await renderer.default(island)(
		component.default,
		props,
		{},
		{
			client: "load",
		},
	);

	const button = island.querySelector("button.counter");
	expect(button.textContent).toBe("Count: 2");
	button.click();
	await new Promise((resolve) => setTimeout(resolve, 0));
	expect(island.querySelector("button.counter").textContent).toBe("Count: 3");
});
