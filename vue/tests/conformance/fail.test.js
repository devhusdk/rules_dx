import { createSSRApp } from "vue";
import { renderToString } from "vue/server-renderer";
import Hello from "./Hello.vue";

describe("Hello.vue", () => {
	test("failing case", async () => {
		const html = await renderToString(createSSRApp(Hello));
		expect(html).toContain("goodbye world");
	});
});
