import type { Shared } from "./shared.js";

const value: Shared = { text: "shared" };

if (value.text !== "shared") {
	throw new Error("unexpected text");
}
