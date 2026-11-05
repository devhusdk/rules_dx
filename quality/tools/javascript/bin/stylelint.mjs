import * as nodeModule from "node:module";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, realpathSync, rmSync, symlinkSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const MANAGED = ["stylelint-config-recess-order", "stylelint-config-standard", "stylelint-order"];

// stylelint resolves extends and plugins from the config directory, then the working directory,
// then the global modules directory. It bypasses module hooks, so dx's copies go in a global
// directory that is only reached when the consumer has none.
const global = mkdtempSync(join(tmpdir(), "dx-stylelint-"));
const modules = join(global, "lib", "node_modules");
mkdirSync(modules, { recursive: true });
const require = nodeModule.createRequire(import.meta.url);
const links = [];
const link = (target, name) => {
  const path = join(modules, name);
  if (!existsSync(path)) {
    symlinkSync(target, path, "dir");
    links.push(path);
  }
};
for (const name of MANAGED) {
  const entry = fileURLToPath(import.meta.resolve(name));
  const marker = `${sep}node_modules${sep}${name}${sep}`;
  const root = entry.slice(0, entry.lastIndexOf(marker) + marker.length - 1);
  link(root, name);
  // The package's own dependencies sit beside it, and are looked up from the link.
  const beside = dirname(root);
  for (const other of readdirSync(beside)) {
    link(join(beside, other), other);
  }
}
process.on("exit", () => {
  for (const path of links) {
    unlinkSync(path);
  }
  rmSync(global, { recursive: true, force: true });
});
process.env.PREFIX = global;

// The launcher preserves symlinks, so a file reached through a link would look for its own
// dependencies beside the link. Resolve from the package's real directory instead.
nodeModule.registerHooks({
  resolve(specifier, context, nextResolve) {
    const parent = context.parentURL?.startsWith("file:") ? fileURLToPath(context.parentURL) : "";
    if (parent.startsWith(modules + sep)) {
      const [first, ...rest] = parent.slice(modules.length + 1).split(sep);
      const parentURL = pathToFileURL(join(realpathSync(join(modules, first)), ...rest)).href;
      return nextResolve(specifier, { ...context, parentURL });
    }
    return nextResolve(specifier, context);
  },
});

const stylelintRoot = dirname(require.resolve("stylelint/package.json"));
await import(pathToFileURL(join(stylelintRoot, "bin", "stylelint.mjs")).href);
