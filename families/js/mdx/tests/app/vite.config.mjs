import mdx from "@mdx-js/rollup";
import { defineConfig } from "vite";

export default defineConfig({
	plugins: [{ enforce: "pre", ...mdx({ jsxImportSource: "preact" }) }],
	oxc: { jsx: { runtime: "automatic", importSource: "preact" } },
	build: { assetsInlineLimit: 0 },
});
