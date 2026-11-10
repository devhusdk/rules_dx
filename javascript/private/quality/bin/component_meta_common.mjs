// Shared pieces of the component metadata launchers.
import fs from "node:fs";
import Module, { createRequire } from "node:module";
import path from "node:path";

const SCHEMA = "dx-component-meta/1";

export class Failure extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

export const posix = (p) => p.split(path.sep).join("/");

export const byName = (a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);

export function clean(text) {
  return String(text ?? "").replace(/\r\n?/g, "\n");
}

export function lineOf(text, offset) {
  let line = 1;
  for (let i = 0; i < offset && i < text.length; i++) {
    if (text.charCodeAt(i) === 10) {
      line++;
    }
  }
  return line;
}

export function installLoadGuard(packageName) {
  const here = createRequire(import.meta.url);
  const resolved = fs.realpathSync(here.resolve(`${packageName}/package.json`));
  const marker = `${path.sep}node_modules${path.sep}`;
  const at = resolved.indexOf(marker);
  if (at < 0) {
    throw new Failure(1, `cannot locate the launcher node_modules from ${resolved}`);
  }
  const root = resolved.slice(0, at + marker.length - 1);
  const launcher = fs.realpathSync(new URL(import.meta.url));
  const original = Module._resolveFilename;
  Module._resolveFilename = function (request, parent, ...rest) {
    const filename = original.call(this, request, parent, ...rest);
    if (path.isAbsolute(filename)) {
      let real = filename;
      try {
        real = fs.realpathSync(filename);
      } catch {}
      if (real !== launcher && !real.startsWith(root + path.sep)) {
        throw new Error(`refusing to load ${filename} from outside the launcher node_modules`);
      }
    }
    return filename;
  };
}

export function parseArgs(argv) {
  const out = { entries: [] };
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (value === undefined || value === "") {
      throw new Failure(1, `${flag} needs a value`);
    }
    if (flag === "--entry") {
      out.entries.push(value);
    } else if (flag === "--tsconfig" || flag === "--out") {
      if (out[flag.slice(2)] !== undefined) {
        throw new Failure(1, `${flag} given more than once`);
      }
      out[flag.slice(2)] = value;
    } else {
      throw new Failure(1, `unknown argument '${flag}'`);
    }
  }
  if (!out.tsconfig || !out.out || out.entries.length === 0) {
    throw new Failure(1, "usage: --tsconfig FILE --out FILE --entry FILE [--entry FILE]...");
  }
  return out;
}

export function readConfig(ts, tsconfigPath) {
  const text = ts.sys.readFile(tsconfigPath);
  if (text === undefined) {
    throw new Failure(1, `cannot read ${tsconfigPath}`);
  }
  const read = ts.readConfigFile(tsconfigPath, ts.sys.readFile);
  if (read.error) {
    throw new Failure(1, ts.flattenDiagnosticMessageText(read.error.messageText, "\n"));
  }
  const files = new Set();
  const host = {
    ...ts.sys,
    readFile(fileName) {
      if (!fileName.endsWith("/package.json")) {
        files.add(fileName);
      }
      return ts.sys.readFile(fileName);
    },
    readDirectory: () => [],
  };
  const parsed = ts.parseJsonConfigFileContent(read.config, host, path.dirname(path.resolve(tsconfigPath)), {}, path.resolve(tsconfigPath));
  const errors = parsed.errors.filter((e) => e.code !== 18003); // no inputs found
  if (errors.length > 0) {
    throw new Failure(1, errors.map((e) => ts.flattenDiagnosticMessageText(e.messageText, "\n")).join("\n"));
  }
  files.add(path.resolve(tsconfigPath));
  return { json: read.config, files };
}

export function relative(file, cwd) {
  const rel = posix(path.relative(cwd, file));
  if (rel === "" || rel.startsWith("..") || path.isAbsolute(rel) || rel.split("/").includes("node_modules")) {
    return "";
  }
  return rel;
}

export function writeMeta(out, framework, tools, components) {
  components.sort(byName);
  const output = { schema: SCHEMA, framework, tools, components };
  fs.writeFileSync(out, JSON.stringify(output, null, 2) + "\n");
}

export async function run(name, main) {
  try {
    await main();
    process.exit(0);
  } catch (error) {
    if (error instanceof Failure) {
      console.error(`${name}: ${error.message}`);
      process.exit(error.code);
    }
    console.error(`${name}: unexpected error`);
    console.error(error);
    process.exit(2);
  }
}
