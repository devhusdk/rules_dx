import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const tool = path.join(here, "mdx_tsc.mjs");

function check(name) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mdx-tsc-"));
  for (const file of [`${name}.mdx`, "lib.ts"]) {
    fs.copyFileSync(path.join(here, "testdata", "mdx", file), path.join(dir, file));
  }
  fs.writeFileSync(
    path.join(dir, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        jsx: "preserve",
        module: "ESNext",
        moduleResolution: "Bundler",
        noEmit: true,
        skipLibCheck: true,
        target: "ES2022",
      },
      files: [`${name}.mdx`],
    }),
  );
  return spawnSync(process.execPath, [tool, "--project", "tsconfig.json", "--pretty", "false"], {
    cwd: dir,
    encoding: "utf8",
  });
}

test("bad.mdx reports MDX positions", () => {
  const result = check("bad");
  assert.notEqual(result.status, 0);
  const lines = result.stdout.split("\n").filter((line) => line.startsWith("bad.mdx("));
  assert.equal(lines.length, 3, result.stdout);
  for (const line of lines) {
    assert.match(line, /^bad\.mdx\(\d+,\d+\): error TS\d+:/);
  }
  assert.match(lines[0], /^bad\.mdx\(1,/);
  assert.match(lines[1], /^bad\.mdx\(3,/);
  assert.match(lines[2], /^bad\.mdx\(7,/);
});

test("clean.mdx passes", () => {
  const result = check("clean");
  assert.equal(result.stdout, "");
  assert.equal(result.status, 0);
});
