import { greet } from "./greet.js";

test("greets by name", () => {
	expect(greet("ada")).toBe("hello ada");
});
