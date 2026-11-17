# Use JetBrains IDEs and other editors

This page helps you point VS Code, JetBrains IDEs and other editors at the
interpreters, SDKs and tools `dx` manages.

## Steps

1. Write the editor settings. `dx init --apply` writes `.vscode/settings.json`
   and `.vscode/extensions.json` for the languages in the repo:

   ```sh
   dx init --apply
   ```

2. Fill the files those settings point at:

   ```sh
   dx sync
   ```

   `dx sync` writes the editor files under `.dx/local/ide/`. It also links
   ruff, biome, shellcheck and clangd into `.dx/local/bin` from the copies
   Bazel already fetched.

3. Open the repo in your editor. Exclude `bazel-*` and `.dx/local/ide/` from
   indexing. The VS Code settings already exclude both from the file watcher
   and search.

## Example: IntelliJ IDEA, GoLand, PyCharm

1. Run `dx sync`.
2. Install the JetBrains Bazel plugin and import the repo.
3. Set the Project SDK to `.dx/local/ide/jdk/home`.
4. Set the Python interpreter to `.dx/local/ide/python/python`.

## CLion

Use Bazel for CLion. Or open `.dx/local/ide/cc/compile_commands.json` as a
compilation database.

## Other editors

- clangd: run it with `--compile-commands-dir=.dx/local/ide/cc`.
- pyright: set the interpreter to `.dx/local/ide/python/python` and add
  `.dx/local/ide/python/site-packages` to `extraPaths`.
- Go: set `GOPACKAGESDRIVER` to the absolute path of
  `.dx/local/ide/go/gopackagesdriver` (`gopackagesdriver.cmd` on Windows). In
  VS Code, set it in `go.toolsEnvVars`.

## Paths

| Path | Used for |
| --- | --- |
| `.dx/local/ide/python/python` | Python interpreter |
| `.dx/local/ide/python/site-packages` | Packages locked in `uv.lock` (from the `@pypi` hub), for `python.analysis.extraPaths` |
| `.dx/local/ide/jdk/home` | Java JDK |
| `.dx/local/ide/dotnet/root` | .NET SDK, for C# and F# |
| `.dx/local/ide/typescript/lib` | TypeScript library |
| `.dx/local/ide/cc/compile_commands.json` | C and C++ compilation database |
| `.dx/local/ide/go/gopackagesdriver` (`gopackagesdriver.cmd` on Windows) | Go packages driver |
| `.dx/local/ide/rust_analyzer/` | rust-analyzer discover and flycheck launchers |

The `@pypi` hub is the Bazel repository that holds the Python packages from
`uv.lock`.

## VS Code settings

The settings use the `.dx/local/bin` links for `ruff.path`, `biome.lsp.bin`,
`shellcheck.executablePath` and `clangd.path`. ruff, biome and shellcheck are
only set when `dx.toml` selects them.

## VS Code extensions

`dx init --apply` recommends one VS Code extension per language with files in
the tree, in `.vscode/extensions.json`. These languages get one too:

| Files | Extension |
| --- | --- |
| `.dart` | `Dart-Code.dart-code` |
| `.astro` | `astro-build.astro-vscode` (also `bradlc.vscode-tailwindcss`) |
| `.hs` | `haskell.haskell` |
| `.ex`, `.exs` | `elixir-lsp.vscode-elixir-ls` |
| `.jl` | `julialang.language-julia` |
| `.bicep`, `.bicepparam` | `ms-azuretools.vscode-bicep` |
| `.cmake`, `CMakeLists.txt` | `ms-vscode.cmake-tools` |
