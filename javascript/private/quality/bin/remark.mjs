import { readFileSync, writeFileSync } from "node:fs";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";
import remarkMdx from "remark-mdx";
import remarkParse from "remark-parse";
import remarkStringify from "remark-stringify";
import { unified } from "unified";

const OPTION_KEYS = [
  "bullet",
  "bulletOther",
  "bulletOrdered",
  "closeAtx",
  "emphasis",
  "fence",
  "fences",
  "incrementListMarker",
  "listItemIndent",
  "quote",
  "resourceLink",
  "rule",
  "ruleRepetition",
  "ruleSpaces",
  "setext",
  "strong",
  "tightDefinitions",
];

const fail = (message) => {
  process.stderr.write(`remark: ${message}\n`);
  process.exit(2);
};

const readSettings = (file) => {
  let value;
  try {
    value = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    fail(`cannot read settings ${file}: ${error.message}`);
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`settings ${file} must be a JSON object`);
  }
  const unknown = Object.keys(value).filter((key) => !OPTION_KEYS.includes(key));
  if (unknown.length > 0) {
    fail(`settings ${file} has keys remark-stringify does not take: ${unknown.join(", ")}`);
  }
  return value;
};

let mode;
let settingsFile;
const files = [];
const args = process.argv.slice(2);
for (let index = 0; index < args.length; index += 1) {
  const arg = args[index];
  if (arg === "--check" || arg === "--write") {
    if (mode !== undefined && mode !== arg) {
      fail("pass only one of --check and --write");
    }
    mode = arg;
  } else if (arg === "--settings") {
    settingsFile = args[index + 1];
    index += 1;
    if (settingsFile === undefined) {
      fail("--settings needs a file");
    }
  } else if (arg.startsWith("--settings=")) {
    settingsFile = arg.slice("--settings=".length);
  } else if (arg.startsWith("-")) {
    fail(`unknown flag ${arg}`);
  } else {
    files.push(arg);
  }
}
if (mode === undefined) {
  fail("pass --check or --write");
}

const defaults = JSON.parse(readFileSync(new URL("./remark_options.json", import.meta.url), "utf8"));
const options = { ...defaults, ...(settingsFile === undefined ? {} : readSettings(settingsFile)) };

const processor = unified()
  .use(remarkParse)
  .use(remarkFrontmatter, ["yaml", "toml"])
  .use(remarkGfm)
  .use(remarkMdx)
  .use(remarkStringify, options)
  .freeze();

const bare = (node) => {
  if (Array.isArray(node)) {
    return node.map(bare);
  }
  if (node === null || typeof node !== "object") {
    return node;
  }
  const out = {};
  for (const key of Object.keys(node).sort()) {
    if (key !== "position" && key !== "data") {
      out[key] = bare(node[key]);
    }
  }
  return out;
};

const located = (file, error) => {
  const reason = String(error?.reason ?? error?.message ?? error).split("\n")[0];
  const spelled = /\((\d+):(\d+)(?:-\d+:\d+)?\)/.exec(reason);
  const start = error?.place?.start ?? error?.place;
  const line = error?.line ?? start?.line ?? (spelled ? Number(spelled[1]) : 1);
  const column = error?.column ?? start?.column ?? (spelled ? Number(spelled[2]) : 1);
  return `${file}:${line}:${column}: ${reason}`;
};

const keepLiteralLinks = (node, input) => {
  if (!Array.isArray(node.children)) {
    return node;
  }
  const children = node.children.map((child) => {
    const start = child.position?.start?.offset;
    const end = child.position?.end?.offset;
    if (child.type === "link" && start !== undefined && end !== undefined) {
      const raw = input.slice(start, end);
      if (!raw.startsWith("[") && !raw.startsWith("<")) {
        return { type: "html", value: raw };
      }
    }
    return keepLiteralLinks(child, input);
  });
  return { ...node, children };
};

const results = [];
const errors = [];
for (const file of files) {
  let input;
  try {
    input = readFileSync(file, "utf8");
  } catch (error) {
    fail(`cannot read ${file}: ${error.message}`);
  }
  let tree;
  try {
    tree = processor.runSync(processor.parse(input));
  } catch (error) {
    errors.push(located(file, error));
    continue;
  }
  const output = processor.stringify(keepLiteralLinks(tree, input));
  let reparsed;
  try {
    reparsed = processor.runSync(processor.parse(output));
  } catch (error) {
    errors.push(`${file}:1:1: would change the document tree (${located(file, error)})`);
    continue;
  }
  if (JSON.stringify(bare(tree)) !== JSON.stringify(bare(reparsed))) {
    errors.push(`${file}:1:1: would change the document tree`);
    continue;
  }
  results.push({ file, formatted: output === input, output });
}

if (errors.length > 0) {
  process.stderr.write(`${errors.join("\n")}\n`);
}
if (mode === "--check") {
  process.stdout.write(`${JSON.stringify(results.map(({ file, formatted }) => ({ file, formatted })))}\n`);
  process.exit(errors.length > 0 ? 2 : results.every((result) => result.formatted) ? 0 : 1);
}
if (errors.length > 0) {
  process.exit(2);
}
for (const result of results) {
  if (!result.formatted) {
    writeFileSync(result.file, result.output);
  }
}
