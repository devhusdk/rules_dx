import { experimental_AstroContainer as AstroContainer } from "astro/container";
import Hello from "./Hello.astro";

describe("Hello.astro", () => {
	test("renders the greeting built by the local helper", async () => {
		const container = await AstroContainer.create();
		const html = await container.renderToString(Hello);
		expect(html).toContain("hello world");
	});
});
