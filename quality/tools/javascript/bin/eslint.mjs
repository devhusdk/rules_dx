import * as nodeModule from "node:module";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

const MANAGED = [
  "@typescript-eslint/parser",
  "astro-eslint-parser",
  "eslint-plugin-astro",
  "eslint-plugin-mdx",
  "eslint-plugin-svelte",
  "eslint-plugin-vue",
  "svelte-eslint-parser",
  "vue-eslint-parser",
];
const NOT_FOUND = ["ERR_MODULE_NOT_FOUND", "MODULE_NOT_FOUND"];

if (typeof nodeModule.registerHooks !== "function") {
  process.stderr.write(`eslint: node ${process.version} lacks module.registerHooks; dx needs node 22.15 or newer\n`);
  process.exit(2);
}

const managed = (specifier) => MANAGED.some((name) => specifier === name || specifier.startsWith(`${name}/`));

nodeModule.registerHooks({
  resolve(specifier, context, nextResolve) {
    try {
      return nextResolve(specifier, context);
    } catch (error) {
      if (!managed(specifier) || !NOT_FOUND.includes(error?.code)) {
        throw error;
      }
      return nextResolve(specifier, { ...context, parentURL: import.meta.url });
    }
  },
});

const eslintRoot = dirname(nodeModule.createRequire(import.meta.url).resolve("eslint/package.json"));
await import(pathToFileURL(join(eslintRoot, "bin", "eslint.js")).href);
