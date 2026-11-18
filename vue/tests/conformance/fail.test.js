import { createSSRApp } from "vue";
import { renderToString } from "vue/server-renderer";
import HelloWorld from "./HelloWorld.vue";

describe("HelloWorld.vue", () => {
	test("failing case", async () => {
		const html = await renderToString(createSSRApp(HelloWorld));
		expect(html).toContain("goodbye world");
	});
});
