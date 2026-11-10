import fs from "node:fs";
import path from "node:path";

const PREFIX = "@graphql-eslint/";
const KEYS = ["schema", "documents", "rules", "baseline"];
const RULE_SETS = ["schema", "operations"];
const SCHEMA_RULES = [
  "known-type-names",
  "known-directives",
  "known-argument-names",
  "provided-required-arguments",
  "unique-type-names",
  "unique-field-definition-names",
  "unique-enum-value-names",
  "unique-directive-names",
  "unique-directive-names-per-location",
  "unique-operation-types",
  "lone-schema-definition",
  "possible-type-extension",
];
const OPERATION_RULES = [
  "executable-definitions",
  "fields-on-correct-type",
  "fragments-on-composite-type",
  "known-argument-names",
  "known-directives",
  "known-fragment-names",
  "known-type-names",
  "lone-anonymous-operation",
  "no-fragment-cycles",
  "no-undefined-variables",
  "no-unused-variables",
  "one-field-subscriptions",
  "overlapping-fields-can-be-merged",
  "possible-fragment-spread",
  "provided-required-arguments",
  "scalar-leafs",
  "unique-argument-names",
  "unique-directive-names-per-location",
  "unique-input-field-names",
  "unique-variable-names",
  "value-literals-of-correct-type",
  "variables-are-input-types",
  "variables-in-allowed-position",
];

class UsageError extends Error {}

function fail(message) {
  throw new UsageError(message);
}

function parseArgs(argv) {
  const separator = argv.indexOf("--");
  if (separator === -1) {
    fail("usage: graphql_eslint [--typecheck] --config <json> --root <dir> -- <files...>");
  }
  const options = {};
  const flags = argv.slice(0, separator);
  const typecheck = flags.includes("--typecheck");
  const pairs = flags.filter((flag) => flag !== "--typecheck");
  for (let index = 0; index < pairs.length; index += 2) {
    const flag = pairs[index];
    const value = pairs[index + 1];
    if ((flag !== "--config" && flag !== "--root") || value === undefined) {
      fail(`unexpected argument ${flag}`);
    }
    options[flag.slice(2)] = value;
  }
  const files = argv.slice(separator + 1);
  if (options.config === undefined || options.root === undefined || files.length === 0) {
    fail("usage: graphql_eslint [--typecheck] --config <json> --root <dir> -- <files...>");
  }
  return { config: options.config, root: options.root, files, typecheck };
}

function pointers(config, key, required) {
  const value = config[key];
  if (value === undefined && !required) {
    return [];
  }
  const list = typeof value === "string" ? [value] : value;
  if (!Array.isArray(list) || list.length === 0 || list.some((item) => typeof item !== "string" || item === "")) {
    fail(`config key ${key} must be a path or a non-empty list of paths`);
  }
  return list.map((pointer) => resolvePointer(key, pointer));
}

function resolvePointer(key, pointer) {
  if (/^[a-z][a-z0-9+.-]*:/i.test(pointer) || path.isAbsolute(pointer)) {
    fail(`${key} pointer ${pointer} must be a workspace-relative path`);
  }
  if (/[*?[\]{}!]/.test(pointer)) {
    fail(`${key} pointer ${pointer} must name a file; globs are not supported`);
  }
  if (pointer.split(/[\\/]/).includes("..")) {
    fail(`${key} pointer ${pointer} must stay inside the workspace`);
  }
  const resolved = path.resolve(pointer);
  if (!fs.statSync(resolved, { throwIfNoEntry: false })?.isFile()) {
    fail(`${key} pointer ${pointer} matches no file`);
  }
  return resolved;
}

function ruleSet(config, name, defaults) {
  const override = config.rules?.[name];
  if (override === undefined) {
    return Object.fromEntries(defaults.map((rule) => [`${PREFIX}${rule}`, "error"]));
  }
  if (override === null || typeof override !== "object" || Array.isArray(override)) {
    fail(`config key rules.${name} must be an object`);
  }
  const foreign = Object.keys(override).find((rule) => !rule.startsWith(PREFIX));
  if (foreign !== undefined) {
    fail(`config rule ${foreign} must start with ${PREFIX}`);
  }
  return override;
}

function readConfig(file) {
  const config = JSON.parse(fs.readFileSync(file, "utf8"));
  if (config === null || typeof config !== "object" || Array.isArray(config)) {
    fail("config must be a JSON object");
  }
  const unknown = Object.keys(config).find((key) => !KEYS.includes(key));
  if (unknown !== undefined) {
    fail(`unknown config key ${unknown}`);
  }
  if (config.baseline !== undefined) {
    fail("config key baseline is not yet supported");
  }
  if (config.rules !== undefined) {
    if (config.rules === null || typeof config.rules !== "object" || Array.isArray(config.rules)) {
      fail("config key rules must be an object");
    }
    const set = Object.keys(config.rules).find((key) => !RULE_SETS.includes(key));
    if (set !== undefined) {
      fail(`unknown config key rules.${set}`);
    }
  }
  return config;
}

async function typecheck(schemaFiles, documentFiles, files) {
  const graphql = await import("graphql");
  const { validateSDL } = await import("graphql/validation/validate.mjs");
  const messages = new Map(files.map((file) => [file, []]));
  const report = (file, ruleId, error) => {
    const location = error.locations?.[0];
    messages.get(file).push({
      ruleId,
      severity: 2,
      message: error.message,
      line: location?.line ?? 1,
      column: location?.column ?? 1,
      ...end(error),
    });
  };
  const end = (error) => {
    const loc = error.nodes?.[0]?.loc;
    if (loc === undefined) {
      return {};
    }
    const { line, column } = graphql.getLocation(loc.source, loc.end);
    return { endLine: line, endColumn: column };
  };
  const fallback = files[0];
  const owner = (error) => {
    const name = error.nodes?.[0]?.loc?.source?.name ?? error.source?.name;
    return messages.has(name) ? name : fallback;
  };
  const parsed = [];
  for (const file of [...schemaFiles, ...documentFiles]) {
    try {
      parsed.push([file, graphql.parse(new graphql.Source(fs.readFileSync(file, "utf8"), file))]);
    } catch (error) {
      if (!(error instanceof graphql.GraphQLError)) {
        throw error;
      }
      report(owner(error), "graphql/syntax", error);
    }
  }
  const schemaDocs = parsed.filter(([file]) => schemaFiles.includes(file)).map(([, doc]) => doc);
  const sdl = { kind: "Document", definitions: schemaDocs.flatMap((doc) => doc.definitions) };
  if (schemaDocs.length === schemaFiles.length) {
    const sdlErrors = validateSDL(sdl);
    for (const error of sdlErrors) {
      report(owner(error), "graphql/validate-sdl", error);
    }
    if (sdlErrors.length === 0) {
      const schema = graphql.buildASTSchema(sdl, { assumeValidSDL: true });
      for (const error of graphql.validateSchema(schema)) {
        report(owner(error), "graphql/validate-schema", error);
      }
      for (const [file, doc] of parsed) {
        if (!messages.has(file) || schemaFiles.includes(file)) {
          continue;
        }
        for (const error of graphql.validate(schema, doc)) {
          report(file, "graphql/validate", error);
        }
      }
    }
  }
  const results = files.map((file) => ({ filePath: file, messages: messages.get(file) }));
  process.stdout.write(JSON.stringify(results));
  return results.some((result) => result.messages.length > 0) ? 1 : 0;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const config = readConfig(path.resolve(args.config));
  const root = path.resolve(args.root);
  const files = args.files.map((file) => path.resolve(file));
  process.chdir(root);
  const schema = pointers(config, "schema", true);
  const missing = files.find((file) => !fs.statSync(file, { throwIfNoEntry: false })?.isFile());
  if (missing !== undefined) {
    fail(`file ${missing} does not exist`);
  }
  const schemaFiles = new Set(schema);
  const documents = [...new Set([...pointers(config, "documents", false), ...files])].filter(
    (file) => !schemaFiles.has(file),
  );
  if (args.typecheck) {
    return typecheck([...schemaFiles], documents, files);
  }
  const schemaRules = ruleSet(config, "schema", SCHEMA_RULES);
  const operationRules = ruleSet(config, "operations", OPERATION_RULES);
  const { ESLint } = await import("eslint");
  const plugin = (await import("@graphql-eslint/eslint-plugin")).default;
  const isSchema = (file) => schemaFiles.has(path.resolve(file));
  const graphQLConfig = { schema, ...(documents.length > 0 && { documents }) };
  const eslint = new ESLint({
    cwd: root,
    overrideConfigFile: true,
    overrideConfig: [
      {
        files: ["**/*.graphql", "**/*.gql"],
        languageOptions: { parser: plugin.parser, parserOptions: { graphQLConfig } },
        plugins: { "@graphql-eslint": plugin },
      },
      { files: [isSchema], rules: schemaRules },
      { files: [(file) => !isSchema(file)], rules: operationRules },
    ],
  });
  const results = await eslint.lintFiles(files);
  const reported = new Map(results.map((result) => [result.filePath, result]));
  for (const file of files) {
    const result = reported.get(file);
    if (result === undefined) {
      fail(`no result for ${file}`);
    }
    if (result.messages.some((message) => message.ruleId === null && message.fatal !== true)) {
      fail(`file ${file} was ignored: ${result.messages[0].message}`);
    }
    for (const message of result.messages) {
      if (message.fatal === true && message.line === undefined) {
        Object.assign(message, { line: 1, column: 1 });
      }
    }
  }
  const formatter = await eslint.loadFormatter("json");
  process.stdout.write(await formatter.format(results));
  return results.some((result) => result.messages.length > 0) ? 1 : 0;
}

try {
  process.exitCode = await main();
} catch (error) {
  process.stderr.write(`graphql_eslint: ${error instanceof UsageError ? error.message : error?.stack ?? error}\n`);
  process.exitCode = 2;
}
