import { h } from "preact";
import { render } from "preact-render-to-string";
import Hello, { label } from "./Hello.mdx";

describe("Hello.mdx", () => {
	test("renders the greeting built by the local helper", () => {
		const html = render(h(Hello, {}));
		expect(label).toBe("hello");
		expect(html).toContain("hello world");
	});
});
