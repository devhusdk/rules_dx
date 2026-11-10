// Isolated MDX metadata launcher for mdx_doc_shard.
//
// MDX files are compiled and parsed, never evaluated. Imports in them are only type-checked.
//
// usage: mdx_component_meta.mjs --tsconfig FILE --out FILE --entry FILE [--entry FILE]...
// Exit codes: 0 ok, 1 usage or configuration error, 2 extraction error.
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { Failure, byName, clean, installLoadGuard, parseArgs, posix, readConfig, relative, run, writeMeta } from "./component_meta_common.mjs";

function esmBlocks(node, out) {
  if (node.type === "mdxjsEsm") {
    out.push(node);
  }
  for (const child of node.children ?? []) {
    esmBlocks(child, out);
  }
  return out;
}

function expressions(node, out) {
  if (node.data?.estree) {
    out.push(node.data.estree);
  }
  for (const attribute of node.attributes ?? []) {
    if (attribute.data?.estree) {
      out.push(attribute.data.estree);
    } else if (attribute.value?.data?.estree) {
      out.push(attribute.value.data.estree);
    }
  }
  for (const child of node.children ?? []) {
    if (node.type !== "mdxjsEsm") {
      expressions(child, out);
    }
  }
  return out;
}

function propReads(tree, found) {
  if (Array.isArray(tree)) {
    for (const item of tree) {
      propReads(item, found);
    }
    return;
  }
  if (tree === null || typeof tree !== "object") {
    return;
  }
  if (
    tree.type === "MemberExpression" &&
    !tree.computed &&
    tree.object?.type === "Identifier" &&
    tree.object.name === "props" &&
    tree.property?.type === "Identifier"
  ) {
    const line = tree.property.loc?.start.line ?? 1;
    const name = tree.property.name;
    if (!found.has(name) || line < found.get(name)) {
      found.set(name, line);
    }
  }
  for (const [key, value] of Object.entries(tree)) {
    if (key !== "loc" && key !== "range") {
      propReads(value, found);
    }
  }
}

function virtualSource(text, blocks) {
  let out = text.replace(/[^\n\r]/g, " ");
  for (const block of blocks) {
    const start = block.position.start.offset;
    const end = block.position.end.offset;
    out = out.slice(0, start) + text.slice(start, end) + out.slice(end);
  }
  return out;
}

function exportsOf(ts, options, file, content, entry) {
  const host = ts.createCompilerHost(options, true);
  const read = host.readFile.bind(host);
  const sourceFile = host.getSourceFile.bind(host);
  host.fileExists = ((exists) => (name) => name === file || exists(name))(host.fileExists.bind(host));
  host.readFile = (name) => (name === file ? content : read(name));
  host.getSourceFile = (name, ...rest) =>
    name === file ? ts.createSourceFile(name, content, options.target ?? ts.ScriptTarget.ES2022, true, ts.ScriptKind.JS) : sourceFile(name, ...rest);
  const program = ts.createProgram([file], options, host);
  const source = program.getSourceFile(file);
  const diagnostics = [...program.getSyntacticDiagnostics(source), ...program.getSemanticDiagnostics(source)].filter(
    (d) => d.category === ts.DiagnosticCategory.Error,
  );
  if (diagnostics.length > 0) {
    const lines = diagnostics.map((d) => {
      const line = d.start === undefined ? 1 : source.getLineAndCharacterOfPosition(d.start).line + 1;
      return `${entry}:${line}: TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, "\n")}`;
    });
    throw new Failure(2, lines.join("\n"));
  }
  const checker = program.getTypeChecker();
  const module = checker.getSymbolAtLocation(source);
  if (!module) {
    return [];
  }
  const found = [];
  for (const symbol of checker.getExportsOfModule(module)) {
    if (symbol.name === "default") {
      continue;
    }
    const target = symbol.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
    const declaration = symbol.declarations?.[0] ?? target.declarations?.[0];
    const type = declaration ? checker.getTypeOfSymbolAtLocation(target, declaration) : checker.getDeclaredTypeOfSymbol(target);
    found.push({
      name: symbol.name,
      description: clean(ts.displayPartsToString(target.getDocumentationComment(checker))),
      type: clean(checker.typeToString(type, undefined, ts.TypeFormatFlags.NoTruncation)),
      tags: target.getJsDocTags(checker).map((t) => ({ name: t.name, text: clean(ts.displayPartsToString(t.text)) })),
      callable: type.getCallSignatures().length > 0,
      line: declaration ? source.getLineAndCharacterOfPosition(declaration.getStart(source)).line + 1 : 1,
    });
  }
  return found;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const cwd = process.cwd();
  installLoadGuard("typescript");
  const ts = (await import("typescript")).default;
  const mdx = await import("@mdx-js/mdx");
  const remarkFrontmatter = (await import("remark-frontmatter")).default;
  const mdxVersion = JSON.parse(
    fs.readFileSync(path.join(path.dirname(createRequire(import.meta.url).resolve("@mdx-js/mdx")), "package.json"), "utf8"),
  ).version;

  const tsconfigPath = path.resolve(args.tsconfig);
  const { json } = readConfig(ts, tsconfigPath);
  const parsed = ts.parseJsonConfigFileContent(json, ts.sys, path.dirname(tsconfigPath), {}, tsconfigPath);
  const options = { ...parsed.options, allowJs: true, checkJs: true, noEmit: true };
  const plugins = [remarkFrontmatter];

  const components = [];
  const seen = new Set();
  for (const entry of args.entries) {
    const file = posix(path.resolve(entry));
    if (!fs.existsSync(file)) {
      throw new Failure(2, `entry ${entry} does not exist`);
    }
    const text = fs.readFileSync(file, "utf8");
    try {
      await mdx.compile({ path: file, value: text }, { remarkPlugins: plugins });
    } catch (error) {
      throw new Failure(2, `${entry}:${error.line ?? 1}: ${error.reason ?? error.message}`);
    }
    const tree = mdx.createProcessor({ remarkPlugins: plugins }).parse({ path: file, value: text });
    const blocks = esmBlocks(tree, []);
    const reads = new Map();
    propReads(expressions(tree, []), reads);

    const name = path.basename(entry).replace(/\.[^.]+$/, "");
    if (seen.has(name)) {
      throw new Failure(2, `${entry}: duplicate component name ${name}`);
    }
    seen.add(name);
    const rel = relative(file, cwd);
    const exposed = exportsOf(ts, options, `${file}.js`, virtualSource(text, blocks), entry).map(({ line, ...rest }) => ({ ...rest, file: rel, line }));
    components.push({
      name,
      display_name: path.basename(entry),
      file: rel,
      line: 1,
      description: "",
      props: [...reads]
        .map(([prop, line]) => ({ name: prop, description: "", type: "unknown", tags: [], file: rel, line, required: false, default: null }))
        .sort(byName),
      events: [],
      slots: [],
      exposed: exposed.sort(byName),
    });
  }
  writeMeta(args.out, "mdx", { "@mdx-js/mdx": mdxVersion, typescript: ts.version }, components);
}

await run("mdx_component_meta", main);
