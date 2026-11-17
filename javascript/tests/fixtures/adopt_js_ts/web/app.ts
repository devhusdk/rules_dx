import { add } from "./util.js";

export function total(xs: number[]): number {
	return xs.reduce((acc, x) => add(acc, x), 0);
}
