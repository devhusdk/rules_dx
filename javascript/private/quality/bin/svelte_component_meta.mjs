// Isolated svelte2tsx launcher for svelte_doc_shard.
//
// Turns each .svelte entry into TypeScript with svelte2tsx, reads the props from the
// TypeScript checker and maps lines back through the source map. It never reads
// svelte.config.js and a module-load guard refuses any module outside its node_modules.
//
// usage: svelte_component_meta.mjs --tsconfig FILE --out FILE --entry FILE [--entry FILE]...
// Exit codes: 0 ok, 1 usage or configuration error, 2 extraction error.
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import {
  Failure,
  byName,
  clean,
  installLoadGuard,
  lineOf,
  parseArgs,
  posix,
  readConfig,
  relative,
  run,
  writeMeta,
} from "./component_meta_common.mjs";

const SHIMS = ["svelte-shims-v4.d.ts", "svelte-jsx-v4.d.ts"];

function readVersion(require, name) {
  return JSON.parse(fs.readFileSync(require.resolve(`${name}/package.json`), "utf8")).version;
}

function unwrap(text) {
  if (!text.startsWith("(") || !text.endsWith(")")) {
    return text;
  }
  let depth = 0;
  for (let i = 0; i < text.length - 1; i++) {
    depth += text[i] === "(" ? 1 : text[i] === ")" ? -1 : 0;
    if (depth === 0) {
      return text;
    }
  }
  return text.slice(1, -1);
}

function errorLine(error) {
  return error?.start?.line ?? error?.loc?.line ?? 0;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const cwd = process.cwd();
  installLoadGuard("svelte2tsx");
  const require = createRequire(import.meta.url);
  const ts = (await import("typescript")).default;
  const { svelte2tsx } = await import("svelte2tsx");
  const { TraceMap, originalPositionFor } = await import("@jridgewell/trace-mapping");
  const tools = {
    svelte: readVersion(require, "svelte"),
    svelte2tsx: readVersion(require, "svelte2tsx"),
    typescript: ts.version,
  };

  const tsconfigPath = path.resolve(args.tsconfig);
  const { json } = readConfig(ts, tsconfigPath);
  const parsed = ts.parseJsonConfigFileContent(json, ts.sys, path.dirname(tsconfigPath), {}, tsconfigPath);
  const options = { ...parsed.options, noEmit: true, allowArbitraryExtensions: true };

  // Generated TypeScript by `<file>.svelte.ts` path.
  const generated = new Map();
  function generate(svelteFile, shown) {
    const key = `${svelteFile}.ts`;
    if (generated.has(key)) {
      return generated.get(key);
    }
    const source = fs.readFileSync(svelteFile, "utf8");
    let result;
    try {
      result = svelte2tsx(source, { filename: svelteFile, isTsFile: true, mode: "ts" });
    } catch (error) {
      throw new Failure(2, `${shown}:${errorLine(error)}: ${clean(error.message)}`);
    }
    const entry = { code: result.code, trace: new TraceMap(result.map) };
    generated.set(key, entry);
    return entry;
  }

  const files = [];
  for (const entry of args.entries) {
    const file = posix(path.resolve(entry));
    if (!fs.existsSync(file)) {
      throw new Failure(2, `entry ${entry} does not exist`);
    }
    generate(file, entry);
    files.push({ entry, file });
  }

  const shimDir = path.dirname(require.resolve("svelte2tsx/package.json"));
  const shims = SHIMS.map((name) => posix(path.join(shimDir, name)));

  const host = ts.createCompilerHost(options);
  const isGenerated = (name) => name.endsWith(".svelte.ts") && fs.existsSync(name.slice(0, -3));
  const readFile = host.readFile.bind(host);
  host.readFile = (name) => (isGenerated(name) ? generate(name.slice(0, -3), name.slice(0, -3)).code : readFile(name));
  const fileExists = host.fileExists.bind(host);
  host.fileExists = (name) => isGenerated(name) || fileExists(name);
  const getSourceFile = host.getSourceFile.bind(host);
  host.getSourceFile = (name, languageVersion, ...rest) => {
    if (isGenerated(name)) {
      return ts.createSourceFile(name, generate(name.slice(0, -3), name.slice(0, -3)).code, languageVersion, true, ts.ScriptKind.TS);
    }
    return getSourceFile(name, languageVersion, ...rest);
  };
  host.resolveModuleNames = (names, containing) =>
    names.map((name) => {
      if (name.endsWith(".svelte")) {
        const target = posix(path.resolve(path.dirname(containing), name)) + ".ts";
        return isGenerated(target) ? { resolvedFileName: target, extension: ts.Extension.Ts } : undefined;
      }
      return ts.resolveModuleName(name, containing, options, host).resolvedModule;
    });

  const program = ts.createProgram([...files.map((f) => `${f.file}.ts`), ...shims], options, host);
  const checker = program.getTypeChecker();

  function original(item, position) {
    const at = item.getLineAndCharacterOfPosition(position);
    const found = originalPositionFor(generated.get(item.fileName).trace, { line: at.line + 1, column: at.character });
    return found.line ?? 0;
  }

  const sourceCache = new Map();
  function locate(declaration) {
    const item = declaration.getSourceFile();
    const start = declaration.getStart(item);
    if (generated.has(item.fileName)) {
      return { file: relative(item.fileName.slice(0, -3), cwd), line: original(item, start) };
    }
    const shown = relative(item.fileName, cwd);
    if (shown === "") {
      return { file: "", line: 0 };
    }
    if (!sourceCache.has(item.fileName)) {
      sourceCache.set(item.fileName, fs.readFileSync(item.fileName, "utf8"));
    }
    return { file: shown, line: lineOf(sourceCache.get(item.fileName), start) };
  }

  function defaultsOf(item) {
    const found = new Map();
    const visit = (node) => {
      if (
        ts.isVariableDeclaration(node) &&
        ts.isObjectBindingPattern(node.name) &&
        node.initializer &&
        ts.isCallExpression(node.initializer) &&
        node.initializer.expression.getText(item) === "$props"
      ) {
        for (const element of node.name.elements) {
          if (element.initializer) {
            found.set((element.propertyName ?? element.name).getText(item), clean(element.initializer.getText(item)));
          }
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(item);
    return found;
  }

  const components = [];
  const seen = new Set();
  for (const { entry, file } of files) {
    const item = program.getSourceFile(`${file}.ts`);
    if (!item) {
      throw new Failure(2, `${entry}: not part of the type-checked program`);
    }
    const diagnostics = [...program.getSyntacticDiagnostics(item), ...program.getSemanticDiagnostics(item)].filter(
      (d) => d.category === ts.DiagnosticCategory.Error,
    );
    if (diagnostics.length > 0) {
      const lines = diagnostics.map(
        (d) => `${entry}:${original(item, d.start ?? 0)}: TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, "\n")}`,
      );
      throw new Failure(2, lines.join("\n"));
    }
    const moduleSymbol = checker.getSymbolAtLocation(item);
    const exported = moduleSymbol && checker.getExportsOfModule(moduleSymbol).find((s) => s.name === "default");
    const signature =
      exported &&
      checker
        .getTypeOfSymbol(exported.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(exported) : exported)
        .getCallSignatures()[0];
    const propsParameter = signature?.parameters[1];
    if (!propsParameter) {
      throw new Failure(2, `${entry}: only Svelte 5 components with $props() are supported`);
    }
    const propsType = checker.getTypeOfSymbol(propsParameter);
    const defaults = defaultsOf(item);
    const props = [];
    const slots = [];
    for (const symbol of checker.getPropertiesOfType(propsType)) {
      if (symbol.name.startsWith("$$")) {
        continue;
      }
      const text = checker.typeToString(checker.getTypeOfSymbol(symbol), undefined, ts.TypeFormatFlags.NoTruncation);
      const type = symbol.flags & ts.SymbolFlags.Optional ? unwrap(text.replace(/ \| undefined$/, "")) : text;
      const declaration = symbol.declarations?.[0];
      const where = declaration ? locate(declaration) : { file: "", line: 0 };
      const member = {
        name: symbol.name,
        description: clean(ts.displayPartsToString(symbol.getDocumentationComment(checker))),
        type: clean(type),
        tags: symbol.getJsDocTags(checker).map((t) => ({ name: t.name, text: clean(ts.displayPartsToString(t.text)) })),
        file: where.file,
        line: where.line,
      };
      if (/^Snippet(<|$)/.test(type)) {
        slots.push(member);
      } else {
        props.push({ ...member, required: (symbol.flags & ts.SymbolFlags.Optional) === 0, default: defaults.get(symbol.name) ?? null });
      }
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
      description: "",
      props: props.sort(byName),
      events: [],
      slots: slots.sort(byName),
      exposed: [],
    });
  }
  writeMeta(args.out, "svelte", tools, components);
}

await run("svelte_component_meta", main);
