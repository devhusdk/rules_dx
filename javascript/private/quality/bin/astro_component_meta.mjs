// Isolated Astro compiler launcher for astro_doc_shard.
//
// Turns each .astro entry into TSX with the Astro compiler, reads the props from the
// TypeScript checker and maps lines back through the source map. Slots come from the
// compiler's parse tree. It never reads astro.config.* and a module-load guard refuses
// any module outside its node_modules.
//
// usage: astro_component_meta.mjs --tsconfig FILE --out FILE --entry FILE [--entry FILE]...
// Exit codes: 0 ok, 1 usage or configuration error, 2 extraction error.
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import {
  Failure,
  byName,
  clean,
  installLoadGuard,
  parseArgs,
  posix,
  readConfig,
  relative,
  run,
  writeMeta,
} from "./component_meta_common.mjs";

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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const cwd = process.cwd();
  installLoadGuard("@astrojs/compiler");
  const require = createRequire(import.meta.url);
  const ts = (await import("typescript")).default;
  const compiler = await import("@astrojs/compiler");
  const { TraceMap, originalPositionFor } = await import("@jridgewell/trace-mapping");
  const tools = {
    "@astrojs/compiler": readVersion(require, "@astrojs/compiler"),
    typescript: ts.version,
  };

  const tsconfigPath = path.resolve(args.tsconfig);
  const { json } = readConfig(ts, tsconfigPath);
  const parsed = ts.parseJsonConfigFileContent(json, ts.sys, path.dirname(tsconfigPath), {}, tsconfigPath);
  const options = { ...parsed.options, noEmit: true, jsx: ts.JsxEmit.Preserve, types: ["astro/client"] };

  const files = [];
  const generated = new Map();
  const names = new Set();
  for (const entry of args.entries) {
    const file = posix(path.resolve(entry));
    if (!fs.existsSync(file)) {
      throw new Failure(2, `entry ${entry} does not exist`);
    }
    const source = fs.readFileSync(file, "utf8");
    const converted = await compiler.convertToTSX(source, { filename: file });
    const problems = converted.diagnostics.filter((d) => d.severity === 1);
    if (problems.length > 0) {
      throw new Failure(2, problems.map((d) => `${entry}:${d.location?.line ?? 0}: ${clean(d.text)}`).join("\n"));
    }
    generated.set(`${file}.tsx`, { code: `${converted.code}\ndeclare const Fragment: any;\n`, trace: new TraceMap(converted.map) });
    const tree = (await compiler.parse(source, { position: true })).ast;
    files.push({ entry, file, tree });
  }

  const host = ts.createCompilerHost(options);
  const readFile = host.readFile.bind(host);
  host.readFile = (name) => generated.get(name)?.code ?? readFile(name);
  const fileExists = host.fileExists.bind(host);
  host.fileExists = (name) => generated.has(name) || fileExists(name);
  const getSourceFile = host.getSourceFile.bind(host);
  host.getSourceFile = (name, languageVersion, ...rest) =>
    generated.has(name)
      ? ts.createSourceFile(name, generated.get(name).code, languageVersion, true, ts.ScriptKind.TSX)
      : getSourceFile(name, languageVersion, ...rest);

  const program = ts.createProgram(
    files.map((f) => `${f.file}.tsx`),
    options,
    host,
  );
  const checker = program.getTypeChecker();

  function original(item, position) {
    const at = item.getLineAndCharacterOfPosition(position);
    const found = originalPositionFor(generated.get(item.fileName).trace, { line: at.line + 1, column: at.character });
    return found.line ?? 0;
  }

  function defaultsOf(item) {
    const found = new Map();
    const visit = (node) => {
      if (
        ts.isVariableDeclaration(node) &&
        ts.isObjectBindingPattern(node.name) &&
        node.initializer &&
        node.initializer.getText(item) === "Astro.props"
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
  for (const { entry, file, tree } of files) {
    const item = program.getSourceFile(`${file}.tsx`);
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
    const propsParameter = exported && checker.getTypeOfSymbol(exported).getCallSignatures()[0]?.parameters[0];
    if (!propsParameter) {
      throw new Failure(2, `${entry}: no component function found`);
    }
    const defaults = defaultsOf(item);
    const props = [];
    for (const symbol of checker.getPropertiesOfType(checker.getTypeOfSymbol(propsParameter))) {
      const text = checker.typeToString(checker.getTypeOfSymbol(symbol), undefined, ts.TypeFormatFlags.NoTruncation);
      const optional = (symbol.flags & ts.SymbolFlags.Optional) !== 0;
      const declaration = symbol.declarations?.[0];
      const inEntry = declaration?.getSourceFile() === item;
      props.push({
        name: symbol.name,
        description: clean(ts.displayPartsToString(symbol.getDocumentationComment(checker))),
        type: clean(optional ? unwrap(text.replace(/ \| undefined$/, "")) : text),
        tags: symbol.getJsDocTags(checker).map((t) => ({ name: t.name, text: clean(ts.displayPartsToString(t.text)) })),
        file: inEntry ? relative(file, cwd) : "",
        line: inEntry ? original(item, declaration.getStart(item)) : 0,
        required: !optional,
        default: defaults.get(symbol.name) ?? null,
      });
    }

    const slots = new Map();
    const visit = (node) => {
      if (node.type === "element" && node.name === "slot") {
        const name = node.attributes.find((a) => a.name === "name")?.value || "default";
        if (!slots.has(name)) {
          slots.set(name, {
            name,
            description: "",
            type: "unknown",
            tags: [],
            file: relative(file, cwd),
            line: node.position?.start.line ?? 0,
          });
        }
      }
      node.children?.forEach(visit);
    };
    visit(tree);

    const name = path.basename(entry).replace(/\.[^.]+$/, "");
    if (components.some((c) => c.name === name)) {
      throw new Failure(2, `${entry}: duplicate component name ${name}`);
    }
    components.push({
      name,
      display_name: path.basename(entry),
      file: relative(file, cwd),
      line: 1,
      description: "",
      props: props.sort(byName),
      events: [],
      slots: [...slots.values()].sort(byName),
      exposed: [],
    });
  }
  writeMeta(args.out, "astro", tools, components);
  compiler.teardown();
}

await run("astro_component_meta", main);
