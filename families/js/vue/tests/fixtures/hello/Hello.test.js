import { parse } from "@vue/compiler-sfc";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const source = fs.readFileSync(path.join(here, "Hello.vue"), "utf8");
const { descriptor, errors } = parse(source, { filename: "Hello.vue" });

describe("Hello.vue", () => {
	test("parses without errors with template/script/style regions", () => {
		expect(errors).toEqual([]);
		expect(descriptor.template).not.toBeNull();
		expect(descriptor.script).not.toBeNull();
		expect(descriptor.styles).toHaveLength(1);
	});

	test("script block imports the local helper", () => {
		expect(descriptor.script.content).toMatch("./helper.js");
	});
});
