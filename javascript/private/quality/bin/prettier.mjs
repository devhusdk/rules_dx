import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PLUGINS = {
  "prettier-plugin-astro": [".astro"],
  "prettier-plugin-svelte": [".svelte"],
  "@prettier/plugin-xml": [".xml", ".xsd", ".xsl", ".xslt", ".wsdl"],
  "prettier-plugin-gherkin": [".feature"],
  "@prettier/plugin-php": [".php"],
};
// prettier-plugin-sh is loaded for Dockerfiles only; shell stays with shfmt.
const DOCKERFILE_PLUGIN = "prettier-plugin-sh";
const RESERVED = ["--config", "--no-config", "--config-precedence", "--plugin", "--ignore-path"];

const DX_CONFIG = "--dx-config";

function fail(message) {
  process.stderr.write(`prettier: ${message}\n`);
  process.exit(2);
}

// dx passes a bound prettier_config as --dx-config=<abs>; it replaces dx's default options.
const dxConfigs = [];
const args = process.argv.slice(2).filter((arg) => {
  if (arg === DX_CONFIG) {
    fail(`${DX_CONFIG} needs a value: ${DX_CONFIG}=<absolute path>`);
  }
  if (!arg.startsWith(`${DX_CONFIG}=`)) {
    return true;
  }
  dxConfigs.push(arg.slice(DX_CONFIG.length + 1));
  return false;
});
if (dxConfigs.length > 1) {
  fail(`${DX_CONFIG} was passed ${dxConfigs.length} times`);
}
const dxConfig = dxConfigs[0];
if (dxConfig !== undefined && (dxConfig === "" || !path.isAbsolute(dxConfig))) {
  fail(`${DX_CONFIG} must be an absolute path, got ${JSON.stringify(dxConfig)}`);
}
const reserved = args.find((arg) => RESERVED.some((flag) => arg === flag || arg.startsWith(`${flag}=`)));
if (reserved !== undefined) {
  process.stderr.write(`prettier: ${reserved.split("=")[0]} is set by dx and cannot be passed\n`);
  process.exit(2);
}
// Avro IDL (.avdl) is not JSON; prettier has no parser for it, so it is never passed on.
const isAvdl = (arg) => !arg.startsWith("-") && arg.toLowerCase().endsWith(".avdl");
// Helm chart templates are Go templates, not YAML or HTML; the Go template plugin rewrites common
// chart idioms into different YAML, so only Chart.yaml, values files and plain YAML are formatted.
// A file is a chart template when a "templates" directory sits next to a Chart.yaml.
function isHelmOnly(arg) {
  if (arg.startsWith("-")) {
    return false;
  }
  const base = path.basename(arg).toLowerCase();
  if (base.endsWith(".tpl") || base.endsWith(".txt") || base === ".helmignore") {
    return true;
  }
  for (let dir = path.dirname(arg); path.basename(dir) !== "" && dir !== path.dirname(dir); dir = path.dirname(dir)) {
    if (path.basename(dir) === "templates" && existsSync(path.join(path.dirname(dir), "Chart.yaml"))) {
      return true;
    }
  }
  return false;
}
const forwarded = args.filter((arg) => !isAvdl(arg) && !isHelmOnly(arg));
if (forwarded.length < args.length && forwarded.every((arg) => arg.startsWith("-"))) {
  process.exit(0);
}
const files = forwarded.filter((arg) => !arg.startsWith("-")).map((arg) => arg.toLowerCase());
const plugins = Object.entries(PLUGINS)
  .filter(([, extensions]) => files.some((file) => extensions.some((extension) => file.endsWith(extension))))
  .map(([name]) => `--plugin=${fileURLToPath(import.meta.resolve(name))}`);
// Mirrors is_dockerfile_name in core/quality/sources.bzl; keep the two in sync.
function isDockerfileName(file) {
  const base = path.basename(file);
  if (base.endsWith(".dockerignore")) {
    return false;
  }
  const stems = ["dockerfile", "containerfile"];
  const suffixes = [".dockerfile", ".containerfile"];
  return stems.some((stem) => base === stem || (base.startsWith(`${stem}.`) && base.length > stem.length + 1)) || suffixes.some((suffix) => base.endsWith(suffix) && base.length > suffix.length);
}
const dockerfiles = files.length > 0 && files.every(isDockerfileName);
if (files.some(isDockerfileName)) {
  plugins.push(`--plugin=${fileURLToPath(import.meta.resolve(DOCKERFILE_PLUGIN))}`);
}
// Prettier cannot infer a parser for names like Dockerfile.prod.
const parser = dockerfiles ? ["--parser=dockerfile"] : [];
// CloudFormation allows a .template extension for either JSON or YAML; prettier cannot infer a
// parser for it, so one is picked from the content when the whole batch agrees.
function templateParser(file) {
  const lower = file.toLowerCase();
  if (lower.endsWith(".json")) {
    return "json";
  }
  if (lower.endsWith(".yaml") || lower.endsWith(".yml")) {
    return "yaml";
  }
  if (!lower.endsWith(".template")) {
    return undefined;
  }
  return readFileSync(file, "utf8").trimStart().startsWith("{") ? "json" : "yaml";
}
const templates = forwarded.filter((arg) => !arg.startsWith("-") && arg.toLowerCase().endsWith(".template"));
if (templates.length > 0) {
  const parsers = new Set(forwarded.filter((arg) => !arg.startsWith("-")).map(templateParser));
  if (parsers.size !== 1 || parsers.has(undefined)) {
    fail(`.template files need a batch of one format (all JSON or all YAML): ${templates.join(", ")}`);
  }
  parser.push(`--parser=${[...parsers][0]}`);
}
const options = `--config=${dxConfig ?? fileURLToPath(new URL("../../../../core/quality/tiers/prettier/standard.json", import.meta.url))}`;
const ignore = `--ignore-path=${fileURLToPath(new URL("./prettier_ignore", import.meta.url))}`;
process.argv.splice(2, args.length, ...plugins, ...parser, options, ignore, ...forwarded);
createRequire(import.meta.url)("prettier/bin/prettier.cjs");
