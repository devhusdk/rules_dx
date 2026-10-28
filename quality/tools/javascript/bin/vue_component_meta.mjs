// Isolated vue-component-meta launcher for vue_doc_shard.
//
// Upstream `createChecker(tsconfig)` and `@vue/language-core` call `require()` on
// every entry of `vueCompilerOptions.plugins`, in the tsconfig and in each file it
// extends, and swallow config errors. This launcher reads the whole config chain
// itself, rejects plugins, and only then hands the JSON to `createCheckerByJson`.
// Never switch to `createChecker(path)`. A module-load guard additionally refuses
// any module outside the launcher's own node_modules.
//
// usage: vue_component_meta.mjs --tsconfig FILE --out FILE --entry FILE [--entry FILE]...
// Exit codes: 0 ok, 1 usage or configuration error, 2 extraction error.
import fs from "node:fs";
import Module, { createRequire } from "node:module";
import path from "node:path";

const SCHEMA = "dx-component-meta/1";

class Failure extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

function installLoadGuard() {
  const here = createRequire(import.meta.url);
  const resolved = fs.realpathSync(here.resolve("vue-component-meta/package.json"));
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

function parseArgs(argv) {
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

const posix = (p) => p.split(path.sep).join("/");

// Reads the tsconfig and every file it extends, failing on any diagnostic or any
// `vueCompilerOptions.plugins`. Returns the JSON for createCheckerByJson.
function readConfig(ts, tsconfigPath) {
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
  for (const file of files) {
    const chained = ts.readConfigFile(file, ts.sys.readFile);
    if (chained.error) {
      throw new Failure(1, `${file}: ${ts.flattenDiagnosticMessageText(chained.error.messageText, "\n")}`);
    }
    const plugins = chained.config?.vueCompilerOptions?.plugins;
    if (plugins !== undefined && !(Array.isArray(plugins) && plugins.length === 0)) {
      throw new Failure(1, `${file}: vueCompilerOptions.plugins is not supported (plugins run arbitrary JavaScript)`);
    }
  }
  return read.config;
}

function relative(file, cwd) {
  const rel = posix(path.relative(cwd, file));
  if (rel === "" || rel.startsWith("..") || path.isAbsolute(rel) || rel.split("/").includes("node_modules")) {
    return "";
  }
  return rel;
}

// Declaration ranges are UTF-16 offsets into the file text; JS strings are UTF-16.
function locate(declarations, cwd, cache) {
  const first = declarations.find((d) => relative(d.file, cwd) !== "");
  if (!first) {
    return { file: "", line: 0 };
  }
  if (!cache.has(first.file)) {
    cache.set(first.file, fs.readFileSync(first.file, "utf8"));
  }
  const content = cache.get(first.file);
  let line = 1;
  for (let i = 0; i < first.range[0] && i < content.length; i++) {
    if (content.charCodeAt(i) === 10) {
      line++;
    }
  }
  return { file: relative(first.file, cwd), line };
}

function clean(text) {
  return String(text ?? "").replace(/\r\n?/g, "\n");
}

function member(item, cwd, cache, extra) {
  const where = locate(item.getDeclarations(), cwd, cache);
  return {
    name: item.name,
    description: clean(item.description),
    type: clean(item.type),
    tags: item.tags.map((t) => ({ name: t.name, text: clean(t.text) })),
    file: where.file,
    line: where.line,
    ...extra,
  };
}

const byName = (a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const cwd = process.cwd();
  installLoadGuard();
  const ts = (await import("typescript")).default;
  const meta = await import("vue-component-meta");
  const metaVersion = JSON.parse(
    fs.readFileSync(createRequire(import.meta.url).resolve("vue-component-meta/package.json"), "utf8"),
  ).version;

  const tsconfigPath = path.resolve(args.tsconfig);
  const json = readConfig(ts, tsconfigPath);
  const checker = meta.createCheckerByJson(posix(path.dirname(tsconfigPath)), json, {
    schema: false,
    printer: { newLine: ts.NewLineKind.LineFeed },
  });

  const program = checker.getProgram();
  const components = [];
  const cache = new Map();
  const seen = new Set();
  for (const entry of args.entries) {
    const file = posix(path.resolve(entry));
    if (!fs.existsSync(file)) {
      throw new Failure(2, `entry ${entry} does not exist`);
    }
    const source = program?.getSourceFile(file);
    if (source) {
      const diagnostics = [...program.getSyntacticDiagnostics(source), ...program.getSemanticDiagnostics(source)].filter(
        (d) => d.category === ts.DiagnosticCategory.Error,
      );
      if (diagnostics.length > 0) {
        const lines = diagnostics.map((d) => `${entry}: TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, "\n")}`);
        throw new Failure(2, lines.join("\n"));
      }
    }
    let result;
    try {
      result = checker.getComponentMeta(file);
    } catch (error) {
      throw new Failure(2, `${entry}: ${error.message}`);
    }
    if (result.type === meta.TypeMeta.Unknown) {
      throw new Failure(2, `${entry}: default export is not a component`);
    }
    const name = path.basename(entry).replace(/\.[^.]+$/, "");
    if (seen.has(name)) {
      throw new Failure(2, `${entry}: duplicate component name ${name}`);
    }
    seen.add(name);
    components.push({
      name,
      display_name: path.basename(entry),
      file: relative(file, cwd),
      line: 1,
      description: clean(result.description),
      props: result.props
        .filter((p) => !p.global)
        .map((p) => member(p, cwd, cache, { required: p.required, default: p.default === undefined ? null : clean(p.default) }))
        .sort(byName),
      events: result.events.map((e) => member(e, cwd, cache, { signature: clean(e.signature) })).sort(byName),
      slots: result.slots.map((s) => member(s, cwd, cache, {})).sort(byName),
      exposed: result.exposed
        .map((x) => member(x, cwd, cache, { callable: x.getTypeObject().getCallSignatures().length > 0 }))
        .sort(byName),
    });
  }
  components.sort(byName);
  const output = {
    schema: SCHEMA,
    framework: "vue",
    tools: { "vue-component-meta": metaVersion, typescript: ts.version },
    components,
  };
  fs.writeFileSync(args.out, JSON.stringify(output, null, 2) + "\n");
}

try {
  await main();
  process.exit(0);
} catch (error) {
  if (error instanceof Failure) {
    console.error(`vue_component_meta: ${error.message}`);
    process.exit(error.code);
  }
  console.error("vue_component_meta: unexpected error");
  console.error(error);
  process.exit(2);
}
