import { toList } from "./gleam.mjs";

export function argumentList() {
  return toList(process.argv.slice(2));
}

export function halt(code) {
  process.exit(code);
}
