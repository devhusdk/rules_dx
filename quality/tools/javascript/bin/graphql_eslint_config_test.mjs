import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const tool = path.join(path.dirname(fileURLToPath(import.meta.url)), "graphql_eslint.mjs");

function run(config) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "graphql-eslint-"));
  const file = path.join(dir, "config.json");
  fs.writeFileSync(file, JSON.stringify(config));
  return spawnSync(process.execPath, [tool, "--config", file, "--root", dir, "--", "a.graphql"], {
    encoding: "utf8",
  });
}

test("unknown config key", () => {
  const result = run({ schema: "schema.graphql", extra: true });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /unknown config key extra/);
});

test("unknown rule set", () => {
  const result = run({ schema: "schema.graphql", rules: { bogus: {} } });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /unknown config key rules\.bogus/);
});
