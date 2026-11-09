# `dx clean`

```sh
dx clean
dx clean --bazel
dx clean --expunge --bazel-startup-option=--output_base=/path/to/base
```

```text
dx clean [--bazel | --expunge]
```

Resets the state dx can rebuild. It runs at once; there is no check mode and no `--apply`. It never touches anything outside `.dx/local/`, so committed `.dx/` files stay. Do not `rm -rf .dx`.

It deletes these entries of `.dx/local/`: `setups`, `environments`, `generated`, `bin`, `bin.next`, `bin.prev`, `ide`, `docs`, `profile`, `results.json`, `fingerprint`, `sync.fingerprint`, and the `leases` no live process holds.

It keeps:

- `fuzz/`: crash reproducers are your data.
- `bench/`: baselines.
- `advisory/`: keeps `dx security --offline` working. Delete it by hand.
- `launcher/`: the running `tools/dx` may be using it.

| Flag | What it does |
| --- | --- |
| `--bazel` | Also run `bazel clean`. |
| `--expunge` | Also run `bazel clean --expunge`. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--build-slots-wait`, `--bazel-startup-option` | Global flags. |

Both Bazel flags honour `--bazel-startup-option`. In a `dx worktree` slot, pass the slot's `--output_base` and only that slot's output base is cleaned.

Scopes: none. A scope is a usage error.

dx waits up to 10 seconds for the commit lock (`.dx/local/.commit.lock`). If another dx command still holds it, `dx clean` exits `1` and names the holder.

Editors lose their tool links until you run `dx sync`, so the output ends with `next: dx sync`.

### Output

Text prints `removed <path> (<n> bytes)` for each entry, a `dx clean: removed <n> paths (<b> bytes)` line, and `next: dx sync`.

JSON emits one `notice` with code `clean_removed` per path, with `path` and the byte size in `message`, then one `clean_removed` notice with the total. A failure emits a `clean_failed` error.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | The commit lock stayed busy, a delete failed, or `bazel clean` failed. |
| `2` | Usage error, including any scope or `--apply`. |
