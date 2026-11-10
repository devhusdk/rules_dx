import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const args = process.argv.slice(2);
const kept = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "-r" && i + 1 < args.length && args[i + 1].endsWith(".rb")) {
    i++;
  } else {
    kept.push(args[i]);
  }
}
process.argv = process.argv.slice(0, 2).concat(kept);

const core = createRequire(require.resolve("asciidoctor/package.json"));
const { Options, Invoker } = core("@asciidoctor/cli");
new Invoker(new Options().parse(process.argv)).invoke();
