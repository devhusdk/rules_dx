# `dx clean`

Use `dx clean` to throw away the local state dx can rebuild, when that state looks stale or broken.

```sh
dx clean
dx clean --bazel
dx clean --expunge --bazel-startup-option=--output_base=/path/to/base
```

Run `dx sync` afterwards. Editors lose their tool links until you do, so the output ends with `next: dx sync`.

## Usage

```text
dx clean [--bazel | --expunge]
```

`dx clean` acts at once. There is no check mode and no `--apply`.

## Flags

| Flag | What it does |
| --- | --- |
| `--bazel` | Also run `bazel clean`. |
| `--expunge` | Also run `bazel clean --expunge`. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

`--bazel` and `--expunge` use your `--bazel-startup-option` values. Bazel keeps its build outputs in an *output base* directory. In a `dx worktree` slot, pass the slot's `--output_base`, and only that slot's output base is cleaned.

## Scopes

None. A scope is a usage error.

## What it removes

`dx clean` only touches `.dx/local/`. Committed files under `.dx/` stay. Do not `rm -rf .dx`.

It deletes these entries of `.dx/local/`:

- `setups`, `environments`, `generated`;
- `bin`, `bin.next`, `bin.prev`;
- `ide`, `docs`, `profile`;
- `results.json`, `fingerprint`, `sync.fingerprint`;
- the entries in `leases` that no running process holds.

It keeps:

- `fuzz/`: your crash reproducers.
- `bench/`: your benchmark baselines.
- `advisory/`: it keeps `dx security --offline` working. Delete it by hand if you want it gone.
- `launcher/`: the running `tools/dx` may be using it.

dx waits up to 10 seconds for the commit lock, `.dx/local/.commit.lock`. If another dx command still holds it, `dx clean` exits `1` and names that command. With `--bazel` or `--expunge`, dx runs Bazel after it removes the local entries.

## Output

Text prints:

```text
removed .dx/local/bin (1024 bytes)
dx clean: removed 1 paths (1024 bytes)
next: dx sync
```

JSON emits one `notice` with code `clean_removed` per path, with `path` set and the byte size in `message`. Then it emits one more `clean_removed` notice with the total. A failure emits a `clean_failed` error.

`dx clean` writes no reports.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | The commit lock stayed busy, a path vanished while dx deleted it, or `bazel clean` failed. |
| `2` | Usage error, including any scope or `--apply`. |
| `3` | A delete failed on an I/O error such as permission denied. |
