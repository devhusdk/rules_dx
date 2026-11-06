# Use JetBrains IDEs and other editors

`dx init --apply` and `dx env --apply` write the files editors need under `.dx/local/ide/`.

| Path | Used for |
| --- | --- |
| `.dx/local/ide/python/python` | Python interpreter |
| `.dx/local/ide/jdk/home` | Java JDK |
| `.dx/local/ide/typescript/lib` | TypeScript library |
| `.dx/local/ide/cc/clangd` | clangd binary |
| `.dx/local/ide/cc/compile_commands.json` | C and C++ compilation database |
| `.dx/local/ide/go/gopackagesdriver` | Go packages driver |

## IntelliJ IDEA, GoLand, PyCharm

Install the JetBrains Bazel plugin and import the repo. Set the Project SDK to `.dx/local/ide/jdk/home`. Set the Python interpreter to `.dx/local/ide/python/python`.

## CLion

Use Bazel for CLion. Or open `.dx/local/ide/cc/compile_commands.json` as a compilation database.

## Other editors

- clangd: run it with `--compile-commands-dir=.dx/local/ide/cc`.
- pyright: set the interpreter to `.dx/local/ide/python/python`.
- Go: set `GOPACKAGESDRIVER` to the absolute path of `.dx/local/ide/go/gopackagesdriver`.

## Indexing

Exclude `bazel-*` and `.dx/local/ide/` from indexing.
