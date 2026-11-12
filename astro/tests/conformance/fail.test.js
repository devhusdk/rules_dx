import { experimental_AstroContainer as AstroContainer } from "astro/container";
import Hello from "./Hello.astro";

describe("Hello.astro", () => {
	test("failing case", async () => {
		const container = await AstroContainer.create();
		const html = await container.renderToString(Hello);
		expect(html).toContain("goodbye world");
	});
});
