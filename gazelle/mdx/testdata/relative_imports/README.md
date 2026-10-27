# Relative MDX Imports

Verifies that relative imports resolve by workspace path.
Two components share the base name `Button` in different directories.
Relative TypeScript and JavaScript imports resolve to their own targets,
and a stylesheet that no target owns adds no dependency.
