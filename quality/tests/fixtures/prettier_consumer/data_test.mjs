import assert from "node:assert";
import { readFileSync } from "node:fs";
import { buildSchema, parse, specifiedRules, validate } from "graphql";
import JSON5 from "json5";
import { parse as parseJsonc } from "jsonc-parser";

const [settings, tsconfig, schemaPath, queryPath, fragmentPath] = process.argv.slice(2);
const text = (path) => readFileSync(path, "utf8");

const parsed = JSON5.parse(text(settings));
assert.ok(parsed !== null && typeof parsed === "object", "json5 object");
assert.throws(() => JSON.parse(text(settings)), "settings.json5 must not be plain JSON");

const errors = [];
const config = parseJsonc(text(tsconfig), errors, { allowTrailingComma: true });
assert.deepStrictEqual(errors, [], "jsonc errors");
assert.ok(config !== null && typeof config === "object", "jsonc object");

const schema = buildSchema(text(schemaPath));
const rules = specifiedRules.filter((rule) => rule.name !== "NoUnusedFragmentsRule");
for (const path of [queryPath, fragmentPath]) {
  const problems = validate(schema, parse(text(path)), rules);
  assert.deepStrictEqual(
    problems.map((problem) => problem.message),
    [],
    `${path} is invalid against the schema`,
  );
}
