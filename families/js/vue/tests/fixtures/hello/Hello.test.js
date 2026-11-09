import { createSSRApp } from "vue";
import { renderToString } from "vue/server-renderer";
import Hello from "./Hello.vue";

describe("Hello.vue", () => {
	test("renders the greeting built by the local helper", async () => {
		const html = await renderToString(createSSRApp(Hello));
		expect(html).toContain("hello world");
	});
});
