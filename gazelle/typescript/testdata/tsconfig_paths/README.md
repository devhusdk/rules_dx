# tsconfig paths aliases

`app/main.ts` imports `@/lib/util`. The root `tsconfig.json` extends
`tsconfig.base.json`, which maps `@/*` to the workspace root. Generation
resolves the alias to `//lib:util` with no `resolve` directive.
