import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);

function fail(message) {
  console.error(`svelte_tsc: ${message}`);
  process.exit(1);
}

const flag = process.argv.indexOf("--project");
const project = flag >= 0 ? process.argv[flag + 1] : undefined;
if (!project) {
  fail("--project <tsconfig> is required");
}

const cli = join(dirname(require.resolve("svelte-check/package.json")), "bin", "svelte-check");
const run = spawnSync(
  process.execPath,
  [cli, "--workspace", ".", "--tsconfig", project, "--output", "machine-verbose", "--threshold", "error"],
  { encoding: "utf8", maxBuffer: 1 << 28 },
);
if (run.error) {
  fail(run.error.message);
}

const lines = [];
let completed = false;
for (const row of run.stdout.split(/\r?\n/)) {
  if (row === "") continue;
  const body = row.replace(/^\d+ /, "");
  if (body.startsWith("START ")) continue;
  if (body.startsWith("COMPLETED ")) {
    completed = true;
    continue;
  }
  let entry;
  try {
    entry = JSON.parse(body);
  } catch {
    fail(`unrecognised svelte-check row: ${row}`);
  }
  if (entry.type !== "ERROR") {
    fail(`unexpected svelte-check row: ${row}`);
  }
  const path = entry.filename ?? entry.fn;
  if (typeof path !== "string" || !entry.start || typeof entry.message !== "string") {
    fail(`malformed svelte-check row: ${row}`);
  }
  const fromTs = entry.source === "ts" || entry.source === "js";
  if (fromTs && !(entry.code > 0)) {
    // svelte-check repeats a Svelte compile error as a code-less TypeScript row.
    continue;
  }
  const message = entry.message.replace(/\s*\r?\n\s*/g, " ");
  const rule = fromTs
    ? `TS${entry.code}`
    : `svelte/${String(entry.code ?? entry.source).toLowerCase().replace(/[^a-z0-9_]+/g, "_")}`;
  lines.push(`${path}(${entry.start.line + 1},${entry.start.character + 1}): error ${rule}: ${message}`);
}

if (!completed) {
  fail(`svelte-check did not complete (exit ${run.status}): ${run.stderr.trim().slice(0, 500)}`);
}
if (lines.length === 0) {
  if (run.status !== 0) fail(`svelte-check exited ${run.status} without diagnostics`);
  process.exit(0);
}
console.log(lines.join("\n"));
process.exit(2);
