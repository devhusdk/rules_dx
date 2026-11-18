# Contributing

This page helps you change rules_dx and get your change merged. rules_dx runs everything through Bazel, the build tool that builds and tests each part of the repo. `tools/dx` is the repo's own copy of `dx`.

Read [AGENTS.md](AGENTS.md) before you change code, and `docs/design/layout.md` before you add or move a language.

## Make a change

1. Get your own checkout, called a slot, on a new branch:

   ```sh
   tools/dx worktree add <branch>
   ```

   Work in the path it prints.

2. Edit the code. Update tests and user docs when behaviour changes. Never edit a generated file by hand; run its generator.

3. Let the formatters and linters fix what they can:

   ```sh
   tools/dx check --apply --affected
   ```

   `--affected` limits the run to the Bazel targets your change reaches. A target is one buildable thing, like a library or a test, named by a label such as `//cli/cli:dx`.

4. Run the tests your change reaches:

   ```sh
   tools/dx test --affected
   ```

5. Check that generated files are up to date:

   ```sh
   bazel run //dx:generate_check
   ```

6. Run the gate before you open the pull request:

   ```sh
   tools/dx check --affected
   ```

7. Push, open the pull request, and free your slot:

   ```sh
   tools/dx worktree release
   ```

Warnings count as errors. Library crates return typed errors; only binaries turn them into exit codes.

To run everything, use `tools/dx test //...`. `//...` means every target in the repo, so this takes a long time.

## Run tests on another platform

You can run a test on macOS or Windows in CI with `evidence.yml`. If you cannot dispatch workflows, comment on the issue or pull request:

```text
/evidence platforms=["macos_arm64"] task=test scope=//pkg/...
```

It dispatches the run and replies with its URL. Each run posts the commit status `evidence/<platform>/<task>/<scope>` on the commit: pending at dispatch, then success, failure or error. Watch it with:

```sh
gh pr checks <n> --watch
```

A second run for the same platform, task and scope on one commit replaces the status. The merge-group `gate` job fails when the newest `evidence/*` status on the pull request head is `pending`, `failure` or `error`.

## Use the remote cache

A remote cache lets Bazel reuse results that other machines already built. Export your BuildBuddy API key and add `--config=bb`:

```sh
export BUILDBUDDY_API_KEY=<key>
tools/dx test --affected -- --config=bb
```

`--config=bb` reads the key from the environment through `tools/buildbuddy-credential-helper.sh` on Linux and macOS. Never put the key in `user.bazelrc` or any other rc file, and never pass `--announce_rc`.

With a read-only key, Bazel warns on every build that it cannot upload results. Use `--config=bb-ro` instead.

## Work in a cloud container

Use these steps when you or an agent start from a fresh Linux checkout, such as a cloud agent container.

1. Build dx once. If the environment has a setup script, put this line in it:

   ```sh
   tools/dx --version
   ```

   With a read-only key in `BUILDBUDDY_API_KEY` and `remote.buildbuddy.io` reachable, write this line to `user.bazelrc` before `tools/dx --version`, so the first build and every later one read the remote cache:

   ```sh
   echo 'common --config=bb-ro' >> user.bazelrc
   ```

   The line holds no key; `--config=bb-ro` reads it from `BUILDBUDDY_API_KEY`.

   When `bazel` is not on `PATH`, `tools/dx` downloads the Bazelisk pinned in `tools/bazelisk/pin.env` from GitHub, checks its sha256 and keeps it in `~/.cache/dx/bazelisk`. That Bazelisk fetches Bazel from GitHub releases too, unless you set `BAZELISK_BASE_URL`. Without the remote cache, on 4 CPUs and 8 GB of memory, the first `tools/dx --version` takes about 9 minutes, including the Bazelisk and Bazel downloads. It uses up to 4.5 GB of memory and about 7 GB of disk.

   The first run needs these hosts: `github.com` and `release-assets.githubusercontent.com` for the Bazelisk and Bazel downloads, and `bcr.bazel.build`, the Bazel module registry. If the network blocks one, `tools/dx` stops with a line that names the host; allow it in the container's network settings.

   GitHub source archives also come from `mirror.bazel.build` and `bcr.cloudflaremirrors.com`. `tools/workspace/downloader.cfg` tries those first, so a proxy that blocks `github.com/<owner>/<repo>/archive/` or `codeload.github.com` needs no other step.

2. Work in a slot, as `AGENTS.md` says. A slot is a second checkout with its own Bazel server and output base. Stop the primary checkout's Bazel server first, so only one server holds memory:

   ```sh
   ~/.cache/dx/bazelisk/*/bazel shutdown
   cd "$(tools/dx worktree add <branch> --create=origin/main --path-only)"
   tools/dx --version
   ```

   Without the remote cache, on 4 CPUs and 15 GB of memory, `worktree add` takes about 5 seconds and the slot's first `tools/dx --version` about 4 minutes. The slot's Bazel server uses about 1.6 GB of memory, and the slot adds about 4.3 GB of disk. If you skip `bazel shutdown`, the primary server stays up for 15 idle minutes and holds about 1.5 GB more.

3. Run one test:

   ```sh
   tools/dx test //cli/path:dx_path_test
   ```

   After step 1, on 4 CPUs and 8 GB of memory, this takes about 11 seconds.

Until dx is built, the hooks in `.claude/settings.json` let commands run without building it. They still stop `dx deploy --apply`.

Without user namespaces, Bazel uses `processwrapper-sandbox` instead of `linux-sandbox` on its own. You need no flag.

Without a key, the remote cache returns no hits, so leave `--config=bb` and `--config=ci` off. With a read-only key, the `user.bazelrc` line from step 1 already turns on `--config=bb-ro`.

## Add a language

```sh
bazel run //tools/new_language -- <lang> [title]
```

It creates `<lang>/` with `defs.bzl`, `extensions.bzl`, `BUILD.bazel`, `lang.json`, `pins.json`, `private/` and `tests/`. Fill in `lang.json`, then refresh the support matrix:

```sh
bazel run //tools/docs/support_matrix:update
```

## Bump a Bazel module

A Bazel module is a dependency declared with `bazel_dep` in `MODULE.bazel`. For every module that consumers get, the oldest supported version (the floor) equals the version rules_dx tests with. Each one has a `<DEP>_VERSION` in `core/modules/versions.bzl`, and a `# floor: equals tested` line sits right above its `bazel_dep`. Tools parse that line, so keep it as written.

To bump one, set both the tested version and the floor:

```sh
tools/dx update --apply bazel:<dep> <version>
tools/dx update --apply --floor --reason "equals tested" bazel:<dep> <version>
```

Then run the check:

```sh
bazel test //core/modules:consumer_deps_test
```

It fails when a floor and its tested version differ, or when a consumer `module_fragment.bazel` file pins a module above its floor. The check matters because rules_dx is never the root module in a consumer's build, so its own `single_version_override`s do not apply there and the consumer resolves the floors.

## Run the quality tests

These cover the `dx check`, `dx lint`, `dx typecheck` and `dx format` machinery in `core/quality/`:

```sh
bazel test //core/quality/testdata:real_results
bazel test //core/quality/testdata:tool_overrides
bazel test //core/quality/testdata:runner_matrix
bazel test //core/quality/testdata:aspects
bazel test //core/quality/testdata:structure
bazel test //core/quality:consumer_policy
bazel test //core/quality:rust_policy
bazel test //core/quality:scripts
bazel test //core/quality:starlark_units
```

## Get a review

Find out whether your pull request needs a second review:

```sh
git diff --name-only origin/main...HEAD |
  bazel run //tools/ci/producers:producers_bin -- --review-paths "$PWD/.github/review-paths.txt"
```

`bazel run` starts the tool in its own directory, so pass the list by its full path. If it prints any paths, add the `needs-review` label. The pull request is not merged until a fresh session (the `reviewer` agent, not the author) posts a comment starting `Review:` that lists each finding as `file:line`, or says it found none. The daily `issue-labels` workflow warns about merged pull requests that skipped this.

## File an issue

Use the bug template and paste the output of `dx status`.

Each issue except pinned guides gets one priority label:

- `P0`: agent dev loop (worktrees, slots, fix, check, CI, launcher, JSON).
- `P1`: bugs in existing features.
- `P2`: improve and finish existing features.
- `P3`: new languages, platforms and features, last.

Other labels:

- `bug`, `enhancement`, `tracking`.
- `needs-host`: needs macOS, Windows, a GPU, or signing secrets.
- `needs-review`: see [Get a review](#get-a-review).
- `api`: the pull request changes the public API snapshot in `api/`. The `api-label` check fails without it.
- `blocked`: waiting on something only the owner can do.
- `model:haiku`, `model:sonnet`, `model:opus` and `model:human` pick which agent lane handles the issue.
- `dogfood`: an extra label, never the only one.

A `blocked` issue has a body line that starts with `Blocked on:`, followed by what it waits for: `#N` issues, `owner/repo#N` issues or pull requests in other repositories, a pull request or issue URL, or the owner action. A bare `#N` always means this repository, so write other repositories in full. The daily `issue-labels` workflow removes the label once every named issue is closed or pull request merged, and comments once on a `blocked` issue without the line.

Filters: [user issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+-label%3Adogfood), [dogfood issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+label%3Adogfood).

## The local dx copy

`.dx/local/launcher` holds a copy (about 50 MB) of the last dx you built. `tools/dx worktree add`, `release` and `hook` run that copy without rebuilding, so they never wait on Bazel. Any other `tools/dx` command, or `bazel build //:dx`, rebuilds dx first when its source changed. Both launchers run dx from the copy, so a running dx never locks `bazel-bin`; `dx.exe.old.*` files may stay while an old dx is still running.

You can delete the copy any time; `tools/dx` recreates it. `tools/dx worktree prune --apply --bases` drops it in free slots.
