# Use JetBrains IDEs and other editors

`dx init --apply` and `dx env --apply` write the files editors need under `.dx/ide/`.

| Path | Used for |
| --- | --- |
| `.dx/ide/python/python` | Python interpreter |
| `.dx/ide/jdk/home` | Java JDK |
| `.dx/ide/typescript/lib` | TypeScript library |
| `.dx/ide/cc/clangd` | clangd binary |
| `.dx/ide/cc/compile_commands.json` | C and C++ compilation database |
| `.dx/ide/go/gopackagesdriver` | Go packages driver |

## IntelliJ IDEA, GoLand, PyCharm

Install the JetBrains Bazel plugin and import the repo. Set the Project SDK to `.dx/ide/jdk/home`. Set the Python interpreter to `.dx/ide/python/python`.

## CLion

Use Bazel for CLion. Or open `.dx/ide/cc/compile_commands.json` as a compilation database.

## Other editors

- clangd: run it with `--compile-commands-dir=.dx/ide/cc`.
- pyright: set the interpreter to `.dx/ide/python/python`.
- Go: set `GOPACKAGESDRIVER` to the absolute path of `.dx/ide/go/gopackagesdriver`.

## Indexing

Exclude `bazel-*` and `.dx/ide/` from indexing.
