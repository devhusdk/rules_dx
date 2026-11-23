import assert from "node:assert";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const files = process.argv.slice(2);
const wasmPath = files.find((f) => f.endsWith(".wasm"));
const mjsPath = files.find((f) => f.endsWith(".mjs"));
const lines = [];
const log = console.log;
console.log = (line) => lines.push(String(line));
const runtime = await import(pathToFileURL(mjsPath).href);
const compiled = await runtime.compile(await readFile(wasmPath));
const instantiated = await compiled.instantiate({});
await instantiated.invokeMain();
console.log = log;
assert.deepStrictEqual(lines, ["hello from dart web"]);
