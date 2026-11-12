import { render } from "svelte/server";
import Hello from "./Hello.svelte";

describe("Hello.svelte", () => {
	test("failing case", () => {
		const { body } = render(Hello);
		expect(body).toContain("goodbye world");
	});
});
