import { suffix } from "./helper.js";

export function message(): string {
	return `entry ${suffix("x")}`;
}

console.log(message());
