import { createRequire } from "node:module";
import { createMdxLanguagePlugin } from "@mdx-js/language-service";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";

const require = createRequire(import.meta.url);
const { runTsc } = require("@volar/typescript/lib/quickstart/runTsc");

runTsc(require.resolve("typescript/lib/tsc"), [".mdx"], () => [
  createMdxLanguagePlugin([remarkFrontmatter, remarkGfm], [], true),
]);
