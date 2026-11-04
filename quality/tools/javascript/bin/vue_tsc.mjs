import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

require("vue-tsc").run(require.resolve("typescript/lib/tsc"));
