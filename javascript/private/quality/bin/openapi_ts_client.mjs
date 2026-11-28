import { readFileSync, writeFileSync } from "node:fs";

const fail = (message) => {
  process.stderr.write(`openapi_ts_client: ${message}\n`);
  process.exit(2);
};

const OPERATIONS = /^(export const OPERATIONS = [\s\S]*?) as const satisfies (Record<string, OperationDescriptor>);$/m;
const CLIENT = /^export const client = (createClient(<[^>(]*>))\(/m;
const BINDINGS = /^export const \{ ([\w$, ]+) \} = client;$/gm;

const rewrite = (source, pattern, replace, what) => {
  if (source.search(pattern) < 0) fail(`no ${what} in the Redocly output`);
  return source.replace(pattern, replace);
};

const isolate = (source) => {
  let out = rewrite(
    source,
    OPERATIONS,
    (_, head, shape) => `${head} as const;\nOPERATIONS satisfies ${shape};`,
    "OPERATIONS table",
  );
  out = rewrite(
    out,
    CLIENT,
    (_, call) => `export const client: ReturnType<typeof ${call}> = ${call}(`,
    "client instance",
  );
  return rewrite(
    out,
    BINDINGS,
    (_, names) =>
      names
        .split(",")
        .map((name) => name.trim())
        .map((name) => `export const ${name}: typeof client.${name} = client.${name};`)
        .join("\n"),
    "client bindings",
  );
};

const [input, output] = process.argv.slice(2);
if (!input || !output) fail("usage: openapi_ts_client INPUT OUTPUT");
writeFileSync(output, isolate(readFileSync(input, "utf8")));
