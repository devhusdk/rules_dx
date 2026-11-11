import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const tool = path.join(process.env.JS_BINARY__RUNFILES, process.env.DX_TSC_GATE);

function run(statuses) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tsc-gate-"));
  const args = [tool, path.join(dir, "gate.ok")];
  statuses.forEach((status, i) => {
    fs.writeFileSync(path.join(dir, `${i}.exit`), `${status}\n`);
    fs.writeFileSync(path.join(dir, `${i}.stdout`), `error ${i}\n`);
    args.push(path.join(dir, `${i}.exit`), path.join(dir, `${i}.stdout`));
  });
  const result = spawnSync(process.execPath, args, { encoding: "utf8" });
  return { dir, result };
}

test("zero statuses write the marker", () => {
  const { dir, result } = run(["0", "0"]);
  assert.equal(result.status, 0);
  assert.ok(fs.existsSync(path.join(dir, "gate.ok")));
});

test("a non-zero status prints its log and fails", () => {
  const { dir, result } = run(["0", "2"]);
  assert.equal(result.status, 1);
  assert.equal(result.stderr, "error 1\n");
  assert.ok(!fs.existsSync(path.join(dir, "gate.ok")));
});
