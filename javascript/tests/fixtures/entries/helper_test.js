import { suffix } from "./helper.js";

test("wraps the tag", () => {
	expect(suffix("x")).toBe("<x>");
});
