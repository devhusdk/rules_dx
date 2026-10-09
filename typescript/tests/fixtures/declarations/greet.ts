import type { Greeting } from "./greeting.js";

export function greet(greeting: Greeting): string {
	return `hello ${greeting.text}`;
}
