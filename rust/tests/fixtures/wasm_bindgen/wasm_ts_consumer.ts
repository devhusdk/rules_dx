import { add, add_via_snippet, greet } from "./wasm_nodejs_types.js";

export function roundtrip(name: string, left: number, right: number): string {
	const greeting: string = greet(name);
	const sum: number = add(left, right) + add_via_snippet(left, right);
	return `${greeting} ${sum}`;
}
