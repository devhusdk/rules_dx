import { render } from "svelte/server";
import Hello from "./Hello.svelte";

describe("Hello.svelte", () => {
	test("renders the greeting built by the local helper", () => {
		const { body } = render(Hello);
		expect(body).toContain("hello world");
	});
});
