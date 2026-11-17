// @ts-expect-error
import { readFileSync } from "node:fs";

declare const process: {
	argv: string[];
	env: Record<string, string | undefined>;
};

console.log(readFileSync(`${process.env.JS_BINARY__RUNFILES}/${process.argv[2]}`, "utf8").trim());
