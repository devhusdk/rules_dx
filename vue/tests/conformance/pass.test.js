import { createSSRApp } from "vue";
import { renderToString } from "vue/server-renderer";
import HelloWorld from "./HelloWorld.vue";

describe("HelloWorld.vue", () => {
	test("renders the greeting built by the local helper", async () => {
		const html = await renderToString(createSSRApp(HelloWorld));
		expect(html).toContain("hello world");
	});
});
