// Seed TypeScript entry; execution reuses the JavaScript binary wrapper
// over the compiled output (see //typescript/rules:defs.bzl). The `.js`
// specifier is the TypeScript ESM idiom: it resolves to `./hello.ts` at
// typecheck and to the compiled `./hello.js` at node runtime. The tsconfig
// forces ESM emit (`module: ESNext`) because the runfiles tree carries the
// root `"type": "module"` package.json.
import { hello } from "./hello.js";

console.log(hello("world"));
