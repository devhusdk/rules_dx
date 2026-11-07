import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, relative, resolve } from "node:path";
import { compile } from "@mdx-js/mdx";
import { createMdxLanguagePlugin } from "@mdx-js/language-service";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";

const require = createRequire(import.meta.url);
const { runTsc } = require("@volar/typescript/lib/quickstart/runTsc");

// The language service skips MDX that does not parse, so compile every listed document first.
async function parseErrors() {
  const flag = process.argv.indexOf("--project");
  if (flag < 0) return [];
  const project = resolve(process.argv[flag + 1]);
  const files = JSON.parse(readFileSync(project, "utf8")).files ?? [];
  const lines = [];
  for (const file of files.filter((name) => name.endsWith(".mdx"))) {
    const path = resolve(dirname(project), file);
    try {
      await compile({ path, value: readFileSync(path, "utf8") }, { remarkPlugins: [remarkFrontmatter, remarkGfm] });
    } catch (error) {
      const rule = String(error.ruleId ?? error.source ?? "compile")
        .toLowerCase()
        .replace(/[^a-z0-9_]+/g, "_");
      const message = String(error.reason ?? error.message).replace(/\s*\r?\n\s*/g, " ");
      const shown = relative(process.cwd(), path).replaceAll("\\", "/");
      lines.push(`${shown}(${error.line ?? 1},${error.column ?? 1}): error mdx/${rule}: ${message}`);
    }
  }
  return lines;
}

const errors = await parseErrors();
for (const line of errors) console.log(line);
process.on("exit", (code) => {
  if (errors.length > 0 && code === 0) process.exitCode = 2;
});

runTsc(require.resolve("typescript/lib/tsc"), [".mdx"], () => [
  createMdxLanguagePlugin([remarkFrontmatter, remarkGfm], [], true),
]);
