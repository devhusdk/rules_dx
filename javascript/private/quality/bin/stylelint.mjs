import * as nodeModule from "node:module";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readlinkSync, rmSync, symlinkSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve, sep } from "node:path";
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
// rules_js patches fs so links leaving its sandbox read as plain files. Keep each link's real
// target here, found by following the store's own links by hand.
const targets = new Map();
const follow = (path) => {
  for (;;) {
    try {
      path = resolve(dirname(path), readlinkSync(path));
    } catch {
      return path;
    }
  }
};
const link = (target, name) => {
  const path = join(modules, name);
  if (!existsSync(path)) {
    targets.set(name, follow(target));
    symlinkSync(targets.get(name), path, "dir");
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
      const parts = parent.slice(modules.length + 1).split(sep);
      const depth = parts[0].startsWith("@") ? 2 : 1;
      let base = targets.get(parts[0]) ?? join(modules, parts[0]);
      if (depth === 2) {
        base = follow(join(base, parts[1]));
      }
      const parentURL = pathToFileURL(join(base, ...parts.slice(depth))).href;
      return nextResolve(specifier, { ...context, parentURL });
    }
    return nextResolve(specifier, context);
  },
});

const stylelintRoot = dirname(require.resolve("stylelint/package.json"));
await import(pathToFileURL(join(stylelintRoot, "bin", "stylelint.mjs")).href);
