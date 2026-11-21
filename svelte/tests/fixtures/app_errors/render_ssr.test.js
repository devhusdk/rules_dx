import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ssr = path.join(
	path.dirname(fileURLToPath(import.meta.url)),
	"throwing/site_ssr",
);

test("the SSR render rejects with the thrown error", async () => {
	const entry = fs.readdirSync(ssr).find((name) => /\.m?js$/.test(name));
	const { renderApp } = await import(pathToFileURL(path.join(ssr, entry)).href);
	expect(() => renderApp()).toThrow("render boom");
});
