import { createRequire } from "node:module";
import { relative } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);

function fail(message) {
  console.error(`astro_tsc: ${message}`);
  process.exit(1);
}

const flag = process.argv.indexOf("--project");
const project = flag >= 0 ? process.argv[flag + 1] : undefined;
if (!project) {
  fail("--project <tsconfig> is required");
}

const ERROR = 1;
const { AstroCheck } = require("@astrojs/language-server");

let result;
try {
  const check = new AstroCheck(process.cwd(), require.resolve("typescript"), project);
  result = await check.lint({});
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
if (result.status !== "completed") {
  fail(`astro check did not complete (status ${result.status})`);
}

const lines = [];
for (const file of result.fileResult) {
  const path = relative(process.cwd(), fileURLToPath(file.fileUrl)).split("\\").join("/");
  const errors = [...file.errors].sort(
    (a, b) => a.range.start.line - b.range.start.line || a.range.start.character - b.range.start.character,
  );
  for (const diagnostic of errors) {
    if (diagnostic.severity !== ERROR) continue;
    const fromTs = diagnostic.source === "ts" || diagnostic.source === "js";
    const code = diagnostic.code;
    const rule =
      fromTs && code > 0
        ? `TS${code}`
        : `astro/${String(code ?? diagnostic.source).toLowerCase().replace(/[^a-z0-9_]+/g, "_")}`;
    const message = diagnostic.message.replace(/\s*\r?\n\s*/g, " ");
    const { line, character } = diagnostic.range.start;
    lines.push(`${path}(${line + 1},${character + 1}): error ${rule}: ${message}`);
  }
}

if (lines.length === 0) process.exit(0);
console.log(lines.join("\n"));
process.exit(2);
