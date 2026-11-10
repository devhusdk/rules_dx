# Use JetBrains IDEs and other editors

`dx sync` writes the files editors need under `.dx/local/ide/`. `dx init --apply` writes `.vscode/settings.json` for the languages in the repo and points at these paths, so run `dx sync` after it.

`dx sync` also links ruff, biome, shellcheck and clangd into `.dx/local/bin` from the copies Bazel already fetched. The settings use them for `ruff.path`, `biome.lsp.bin`, `shellcheck.executablePath` and `clangd.path`. ruff, biome and shellcheck are only set when `dx.toml` selects them.

| Path | Used for |
| --- | --- |
| `.dx/local/ide/python/python` | Python interpreter |
| `.dx/local/ide/python/site-packages` | Packages locked in `uv.lock` (from the `@pypi` hub), for `python.analysis.extraPaths` |
| `.dx/local/ide/jdk/home` | Java JDK |
| `.dx/local/ide/typescript/lib` | TypeScript library |
| `.dx/local/ide/cc/compile_commands.json` | C and C++ compilation database |
| `.dx/local/ide/go/gopackagesdriver` (`gopackagesdriver.cmd` on Windows) | Go packages driver |

## IntelliJ IDEA, GoLand, PyCharm

Install the JetBrains Bazel plugin and import the repo. Set the Project SDK to `.dx/local/ide/jdk/home`. Set the Python interpreter to `.dx/local/ide/python/python`.

## CLion

Use Bazel for CLion. Or open `.dx/local/ide/cc/compile_commands.json` as a compilation database.

## Other editors

- clangd: run it with `--compile-commands-dir=.dx/local/ide/cc`.
- pyright: set the interpreter to `.dx/local/ide/python/python` and add `.dx/local/ide/python/site-packages` to `extraPaths`.
- Go: set `GOPACKAGESDRIVER` to the absolute path of `.dx/local/ide/go/gopackagesdriver` (`gopackagesdriver.cmd` on Windows).

## VS Code extensions

`dx init --apply` recommends one VS Code extension per language with files in the tree, in `.vscode/extensions.json`. These languages get one too:

| Files | Extension |
| --- | --- |
| `.dart` | `Dart-Code.dart-code` |
| `.astro` | `astro-build.astro-vscode` (also `bradlc.vscode-tailwindcss`) |
| `.hs` | `haskell.haskell` |
| `.ex`, `.exs` | `elixir-lsp.vscode-elixir-ls` |
| `.jl` | `julialang.language-julia` |
| `.bicep`, `.bicepparam` | `ms-azuretools.vscode-bicep` |
| `.cmake`, `CMakeLists.txt` | `ms-vscode.cmake-tools` |

## Indexing

Exclude `bazel-*` and `.dx/local/ide/` from indexing.
