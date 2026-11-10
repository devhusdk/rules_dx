# Quality config design

This page fixes the rules for the lint and format configuration that dx ships. Tier definitions, the catalog audit and the overlap test cite it.

## Tiers

- `standard` covers correctness, likely bugs, security, deprecated APIs and obvious performance problems. It stays near the upstream "recommended" set. It has no style rules.
- `strict` enables every stable rule except preview rules, deprecated rules, rules that conflict with the formatter, all but one rule of each contradicting pair, and rules that depend on project input. Type checkers run in their strict modes.
- Tier files list their rules explicitly and are self-contained. No tier extends another.
- Formatters run on tool defaults in both tiers.

## Severity

- `error`: type errors, compile errors, security findings and certain bugs.
- `warning`: likely bugs, suspicious code, performance, maintainability and style (naming, readability, pedantic, codestyle).
- `info`: only what the tool itself labels a suggestion or hint, such as hlint `Suggestion`, staticcheck `QF` and vale `suggestion`.
- Severity is mapped once per tool in `core/quality/tools.bzl`. A mapping never soft-disables a rule.

## Profiles

- Each tool has one base profile.
- A tool shared by several families uses the root profile.
- `dx.toml` is rejected when families that share a tool disagree about it.

## Rule IDs

- Tool rules are `tool/<native code>`.
- dx checks are `dx/<check>/<kind>`.
- Swapping the tool behind a language is a breaking change. `dx upgrade` flags it.

## Overlap

- Each concern has one owner per family.
- Each file type has one formatter. A linter that formats is that formatter.
- CI enforces both rules.

## Fixes

- `--apply` applies safe fixes only.
- `--unsafe` opts in to unsafe fixes. The mapping to each tool's own flag lives with the tool.

## Inclusion and levels

- The curated definitions and the catalog definitions each list rules with a level.
- A rule is removed from a tier when the tool deprecates it, moves it to preview, or it starts to conflict with the formatter or another enabled rule.
- A removed rule is deleted from the tier file. It is not left disabled.

## Files

- A native root config file, when the user has one, wins.
- Otherwise dx writes the base config to `.dx/config/<tool>/base.*`.
- A user override is a copy next to the base. On `dx upgrade` the copy is merged with the new base using a 3-way merge.
- `.dx/` is committed. `.dx/local/` is gitignored and holds machine-local settings.

## Security scope and advisory data

- Static analysis and infrastructure-as-code checks are lint.
- Dependency, secret and image scans belong to `dx security`.
- SBOM is a report format, not a check.
- Advisory snapshots are pinned in `.dx/pins` and published to `devhusdk/rules_dx-advisory`.

## Schema versioning

- Every config file has a required `schema_version`.
- Adding a key does not change the version.
- The current and the previous version are accepted.
