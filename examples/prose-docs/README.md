# prose-docs

Documents in four families, linted and formatted by `dx`: reStructuredText and AsciiDoc through Vale, Typst through typstyle, LaTeX through latexindent.

| Package | Contents |
| --- | --- |
| `shared` | One `rst_library` and one `asciidoc_library` that other packages depend on. |
| `guide` | Two `.rst` pages that depend on `shared`. |
| `handbook` | Two `.adoc` pages that depend on `shared`. |
| `paper` | A Typst document with an imported file, and a LaTeX document with an input file. |
| `drafts` | Files with findings. Every target is tagged `manual`, so `bazel build //...` skips them. |

The policy in `BUILD.bazel` selects the tools. `docs.vale.ini` binds a non-default Vale config. `latexindent.yaml` binds the LaTeX indent config. Both are attached with `aspect_hints`.

## Check

```sh
dx lint //examples/prose-docs/... -- --@rules_dx//config:workspace=//examples/prose-docs:policy
dx format //examples/prose-docs/... -- --@rules_dx//config:workspace=//examples/prose-docs:policy
bazel test //examples/prose-docs:bundle_test
```

Both commands exit 0 on the clean packages and never edit files.

## Findings

`drafts` shows what a failure looks like. Name its targets to run them:

```sh
dx lint //examples/prose-docs/drafts:todo_rst -- --@rules_dx//config:workspace=//examples/prose-docs:policy
dx format //examples/prose-docs/drafts:messy -- --@rules_dx//config:workspace=//examples/prose-docs:policy
```

`dx lint` exits 1 and names `drafts/todo.rst`. `dx format` exits 1 and names `drafts/messy.typ`. Add `--apply` to `dx format` to rewrite that file.

## Platforms

| Platform | Status |
| --- | --- |
| Linux x86_64 | Supported |
| Linux arm64 | Not supported yet |
| macOS | Not supported yet |
| Windows | Not supported yet |
