import { run } from "asciidoctor/cli";

const args = process.argv.slice(2);
const kept = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "-r" && i + 1 < args.length && args[i + 1].endsWith(".rb")) {
    i++;
  } else {
    kept.push(args[i]);
  }
}
await run(process.argv.slice(0, 2).concat(kept));
