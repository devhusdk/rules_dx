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
import { createRequire } from "node:module";
import path from "node:path";
import { Failure, byName, clean, installLoadGuard, lineOf, parseArgs, posix, readConfig, relative, run, writeMeta } from "./component_meta_common.mjs";

function checkPlugins(ts, files) {
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
  return { file: relative(first.file, cwd), line: lineOf(content, first.range[0]) };
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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const cwd = process.cwd();
  installLoadGuard("vue-component-meta");
  const ts = (await import("typescript")).default;
  const meta = await import("vue-component-meta");
  const metaVersion = JSON.parse(
    fs.readFileSync(createRequire(import.meta.url).resolve("vue-component-meta/package.json"), "utf8"),
  ).version;

  const tsconfigPath = path.resolve(args.tsconfig);
  const { json, files } = readConfig(ts, tsconfigPath);
  checkPlugins(ts, files);
  const checker = meta.createCheckerByJson(posix(path.dirname(tsconfigPath)), json, {
    schema: false,
    printer: { newLine: ts.NewLineKind.LineFeed },
  });

  const components = [];
  const cache = new Map();
  const seen = new Set();
  for (const entry of args.entries) {
    const file = posix(path.resolve(entry));
    if (!fs.existsSync(file)) {
      throw new Failure(2, `entry ${entry} does not exist`);
    }
    let result;
    try {
      result = checker.getComponentMeta(file);
    } catch (error) {
      throw new Failure(2, `${entry}: ${error.message}`);
    }
    // getComponentMeta adds the entry as a root of the program when the tsconfig
    // does not include it, so the program is fetched again afterwards.
    const checked = checker.getProgram();
    const source = checked?.getSourceFile(file);
    if (!source) {
      throw new Failure(2, `${entry}: not part of the type-checked program, cannot verify it has no type errors`);
    }
    const diagnostics = [...checked.getSyntacticDiagnostics(source), ...checked.getSemanticDiagnostics(source)].filter(
      (d) => d.category === ts.DiagnosticCategory.Error,
    );
    if (diagnostics.length > 0) {
      const lines = diagnostics.map((d) => `${entry}: TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, "\n")}`);
      throw new Failure(2, lines.join("\n"));
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
  writeMeta(args.out, "vue", { "vue-component-meta": metaVersion, typescript: ts.version }, components);
}

await run("vue_component_meta", main);
