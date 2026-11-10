import { suffix } from "./helper.js";

export function message() {
	return `entry ${suffix("x")}`;
}

console.log(message());
