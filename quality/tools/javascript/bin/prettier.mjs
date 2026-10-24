import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const PLUGINS = ["prettier-plugin-sql", "@prettier/plugin-xml", "prettier-plugin-gherkin"];

const plugins = PLUGINS.map((name) => `--plugin=${fileURLToPath(import.meta.resolve(name))}`);
const dialects = `--config=${fileURLToPath(new URL("./sql_dialects.json", import.meta.url))}`;
const args = process.argv.slice(2).map((arg) => (arg === "--no-config" ? dialects : arg));
process.argv.splice(2, args.length, ...plugins, ...args);
createRequire(import.meta.url)("prettier/bin/prettier.cjs");
