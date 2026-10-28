import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PLUGINS = {
  "prettier-plugin-astro": [".astro"],
  "prettier-plugin-sql": [".sql"],
  "prettier-plugin-svelte": [".svelte"],
  "@prettier/plugin-xml": [".xml", ".xsd", ".xsl", ".xslt", ".wsdl"],
  "prettier-plugin-gherkin": [".feature"],
  "@prettier/plugin-php": [".php"],
};
// prettier-plugin-sh is loaded for Dockerfiles only; shell stays with shfmt.
const DOCKERFILE_PLUGIN = "prettier-plugin-sh";
const RESERVED = ["--config", "--no-config", "--config-precedence", "--plugin", "--ignore-path", "--language"];

const DX_CONFIG = "--dx-config";
const DX_SQL_DIALECT = "--dx-sql-dialect";

function fail(message) {
  process.stderr.write(`prettier: ${message}\n`);
  process.exit(2);
}

// dx passes a bound prettier_config as --dx-config=<abs>; it replaces dx's default options.
const dxConfigs = [];
const dxDialects = [];
const args = process.argv.slice(2).filter((arg) => {
  if (arg === DX_CONFIG) {
    fail(`${DX_CONFIG} needs a value: ${DX_CONFIG}=<absolute path>`);
  }
  if (arg === DX_SQL_DIALECT) {
    fail(`${DX_SQL_DIALECT} needs a value: ${DX_SQL_DIALECT}=<dialect>`);
  }
  if (arg.startsWith(`${DX_SQL_DIALECT}=`)) {
    dxDialects.push(arg.slice(DX_SQL_DIALECT.length + 1));
    return false;
  }
  if (!arg.startsWith(`${DX_CONFIG}=`)) {
    return true;
  }
  dxConfigs.push(arg.slice(DX_CONFIG.length + 1));
  return false;
});
if (dxDialects.length > 1) {
  fail(`${DX_SQL_DIALECT} was passed ${dxDialects.length} times`);
}
const dxDialect = dxDialects[0];
if (dxDialect === "") {
  fail(`${DX_SQL_DIALECT} must not be empty`);
}
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
const files = args.filter((arg) => !arg.startsWith("-")).map((arg) => arg.toLowerCase());
const plugins = Object.entries(PLUGINS)
  .filter(([, extensions]) => files.some((file) => extensions.some((extension) => file.endsWith(extension))))
  .map(([name]) => `--plugin=${fileURLToPath(import.meta.resolve(name))}`);
// Mirrors is_dockerfile_name in quality/sources.bzl; keep the two in sync.
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
const language = dxDialect !== undefined && plugins.some((plugin) => plugin.includes("prettier-plugin-sql")) ? [`--language=${dxDialect}`] : [];
const options = `--config=${dxConfig ?? fileURLToPath(new URL("./prettier_options.json", import.meta.url))}`;
const ignore = `--ignore-path=${fileURLToPath(new URL("./prettier_ignore", import.meta.url))}`;
process.argv.splice(2, args.length, ...plugins, ...parser, ...language, options, ignore, ...args);
createRequire(import.meta.url)("prettier/bin/prettier.cjs");
