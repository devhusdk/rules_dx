import { readFileSync } from "node:fs";

console.log(readFileSync(`${process.env.JS_BINARY__RUNFILES}/${process.argv[2]}`, "utf8").trim());
