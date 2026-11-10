import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, relative, resolve } from "node:path";

const require = createRequire(import.meta.url);
const core = createRequire(createRequire(require.resolve("vue-tsc/package.json")).resolve("@vue/language-core"));
const compiler = core("@vue/compiler-dom");

// vue-tsc recovers from a malformed SFC without a diagnostic, so parse every listed file first.
function parseErrors() {
  const flag = process.argv.indexOf("--project");
  if (flag < 0) return [];
  const project = resolve(process.argv[flag + 1]);
  const files = JSON.parse(readFileSync(project, "utf8")).files ?? [];
  const lines = [];
  for (const file of files.filter((name) => name.endsWith(".vue"))) {
    const path = resolve(dirname(project), file);
    const shown = relative(process.cwd(), path).replaceAll("\\", "/");
    compiler.parse(readFileSync(path, "utf8"), {
      comments: true,
      isNativeTag: () => true,
      isPreTag: () => true,
      parseMode: "sfc",
      onError: (error) => {
        const rule = String(compiler.ErrorCodes?.[error.code] ?? `code_${error.code}`).toLowerCase();
        const start = error.loc?.start ?? { line: 1, column: 1 };
        lines.push(`${shown}(${start.line},${start.column}): error vue/${rule}: ${error.message}`);
      },
    });
  }
  return lines;
}

const errors = parseErrors();
for (const line of errors) console.log(line);
process.on("exit", (code) => {
  if (errors.length > 0 && code === 0) process.exitCode = 2;
});

require("vue-tsc").run(require.resolve("typescript/lib/tsc"));
