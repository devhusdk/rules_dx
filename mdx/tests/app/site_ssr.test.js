/**
 * @jest-environment jsdom
 * @jest-environment-options {"customExportConditions": ["node", "node-addons"]}
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const ssr = path.join(here, "site_ssr");
const site = path.join(here, "site");

let meta;
beforeAll(async () => {
	const entry = fs.readdirSync(ssr).find((name) => /\.m?js$/.test(name));
	const module = await import(pathToFileURL(path.join(ssr, entry)).href);
	meta = module.meta;
	document.body.innerHTML = `<div id="app">${module.render()}</div>`;
});

describe("mdx_build SSR", () => {
	test("renders the exported title", () => {
		expect(document.querySelector("h1").textContent).toBe(meta.title);
		expect(meta.title).toBe("Hello from MDX");
	});

	test("renders each imported MDX card with its props", () => {
		const cards = [...document.querySelectorAll(".cards .card")];
		expect(cards.map((card) => card.querySelector("h2").textContent)).toEqual([
			"First",
			"Second",
		]);
		expect(
			cards.map((card) => card.querySelector(".body").textContent),
		).toEqual(["One card.", "Two cards."]);
	});

	test("points the image at an emitted asset", () => {
		const src = document.querySelector("img").getAttribute("src");
		expect(src).toMatch(/^\/assets\/logo-[\w-]+\.png$/);
		expect(fs.existsSync(path.join(site, src))).toBe(true);
	});

	test("keeps the fenced import as code text", () => {
		expect(document.querySelector("pre code").textContent.trim()).toBe(
			'import fake from "./fake.js";',
		);
	});

	test("renders the counter at its start value", () => {
		expect(document.querySelector("button.counter").textContent).toBe(
			"Count: 2",
		);
	});
});
