import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const PLUGINS = {
  "prettier-plugin-astro": [".astro"],
  "prettier-plugin-sql": [".sql"],
  "prettier-plugin-svelte": [".svelte"],
  "@prettier/plugin-xml": [".xml", ".xsd", ".xsl", ".xslt", ".wsdl"],
  "prettier-plugin-gherkin": [".feature"],
};
const RESERVED = ["--config", "--no-config", "--config-precedence", "--plugin", "--ignore-path"];

const args = process.argv.slice(2);
const reserved = args.find((arg) => RESERVED.some((flag) => arg === flag || arg.startsWith(`${flag}=`)));
if (reserved !== undefined) {
  process.stderr.write(`prettier: ${reserved.split("=")[0]} is set by dx and cannot be passed\n`);
  process.exit(2);
}
const files = args.filter((arg) => !arg.startsWith("-")).map((arg) => arg.toLowerCase());
const plugins = Object.entries(PLUGINS)
  .filter(([, extensions]) => files.some((file) => extensions.some((extension) => file.endsWith(extension))))
  .map(([name]) => `--plugin=${fileURLToPath(import.meta.resolve(name))}`);
const options = `--config=${fileURLToPath(new URL("./prettier_options.json", import.meta.url))}`;
const ignore = `--ignore-path=${fileURLToPath(new URL("./prettier_ignore", import.meta.url))}`;
process.argv.splice(2, args.length, ...plugins, options, ignore, ...args);
createRequire(import.meta.url)("prettier/bin/prettier.cjs");
