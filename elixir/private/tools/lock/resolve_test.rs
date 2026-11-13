//! The hex lock step writes the golden lock from a `mix.lock` or `rebar.lock` with an empty `PATH` and no network.

use dx_lock::{Args, Error, Fetch, Outcome, run};
use hex_lock::Hex;

const MIX_EXS: &str = "defmodule Bench.MixProject do
  use Mix.Project

  def project, do: [app: :bench, version: \"0.1.0\", deps: [{:benchee, \"~> 1.5\"}]]
end
";

const MIX_LOCK: &str = r#"%{
  "benchee": {:hex, :benchee, "1.5.1", "b95cbc36c4b98969a5c592a246e171041eb683c56bad1cb4f49a3b081ba66087", [:mix], [{:deep_merge, "~> 1.0", [hex: :deep_merge, repo: "hexpm", optional: false]}, {:statistex, "~> 1.1", [hex: :statistex, repo: "hexpm", optional: false]}, {:table, "~> 0.1.0", [hex: :table, repo: "hexpm", optional: true]}], "hexpm", "a539301f8dfd4efc5c5123bfb9d47ebde20092a863a5b5b16c2a60d2243dfce7"},
  "deep_merge": {:hex, :deep_merge, "1.0.2", "476aa7ea61c54de96220051b998d893869069094da65b96101aebf79416f8a1e", [:mix], [], "hexpm", "737a53cdc9758fedbb608bdc213969e65729466c4ef3cd8e8726d0335dff116c"},
  "statistex": {:hex, :statistex, "1.1.1", "73612aa7f79e53c30569be065fd121e380f1cf57bc4c2da5b41be9246da18df9", [:mix], [], "hexpm", "310c4b49b34adf683de3103639006bed233ab54c08a4add65a531448e653857c"},
}
"#;

const REBAR_CONFIG: &str = "{deps, [cowlib, jsx]}.
";

const REBAR_LOCK: &str = r#"{"1.2.0",
[{<<"cowlib">>,{pkg,<<"cowlib">>,<<"2.12.1">>},0},
 {<<"jsx">>,{pkg,<<"jsx">>,<<"3.1.0">>},0}]}.
[
{pkg_hash,[
 {<<"cowlib">>, <<"A9FA9A625F1D2025FE6B462CB865881329B5CAFF8F1854D1CBC9F9533F00E1E1">>},
 {<<"jsx">>, <<"D12516BAA0BB23A59BB35DCCAF02A1BD08243FCBB9EFE24F2D9D056CCFF71268">>}]},
{pkg_hash_ext,[
 {<<"cowlib">>, <<"163B73F6367A7341B33C794C4E88E7DBFE6498AC42DCD69EF44C5BC5507C8DB0">>},
 {<<"jsx">>, <<"0C5CC8FDC11B53CC25CF65AC6705AD39E54ECC56D1C22E4ADB8F5A53FB9427F3">>}]}
].
"#;

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn set(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("set directory");
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).expect("set file");
    }
    dir
}

fn lock(dir: &std::path::Path) -> Result<Outcome, Error> {
    run(
        &Hex::spec(),
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Hex,
        &NoFetch,
    )
}

fn golden(dir: &std::path::Path, env: &str, rel: &str) {
    dx_testing::snapshot_diff(
        &dx_testing::env_file(env),
        &dir.join(Hex::spec().lock_name()),
        Some(rel),
    )
    .expect("the lock matches the golden");
}

#[test]
fn a_mix_lock_writes_the_golden_lock() {
    let dir = set(&[("mix.exs", MIX_EXS), ("mix.lock", MIX_LOCK)]);
    assert_eq!(lock(dir.path()), Ok(Outcome::Written));
    golden(
        dir.path(),
        "MIX_GOLDEN",
        "elixir/private/tools/lock/testdata/mix.lock.json",
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("mix.lock")).expect("mix.lock"),
        MIX_LOCK
    );
}

#[test]
fn a_rebar_only_set_writes_the_golden_lock() {
    let dir = set(&[("rebar.config", REBAR_CONFIG), ("rebar.lock", REBAR_LOCK)]);
    assert_eq!(lock(dir.path()), Ok(Outcome::Written));
    golden(
        dir.path(),
        "REBAR_GOLDEN",
        "elixir/private/tools/lock/testdata/rebar.lock.json",
    );
}

#[test]
fn an_unchanged_set_is_fresh() {
    let dir = set(&[("mix.exs", MIX_EXS), ("mix.lock", MIX_LOCK)]);
    assert_eq!(lock(dir.path()), Ok(Outcome::Written));
    assert_eq!(lock(dir.path()), Ok(Outcome::Fresh));
}

#[test]
fn a_set_without_state_fails() {
    let dir = set(&[("mix.exs", MIX_EXS)]);
    let error = lock(dir.path()).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("the set has no mix.lock or rebar.lock"),
        "{error}"
    );
}

#[test]
fn a_git_mix_entry_fails() {
    let text = MIX_LOCK.replace(
        "\"deep_merge\": {:hex, :deep_merge,",
        "\"deep_merge\": {:git, :deep_merge,",
    );
    let dir = set(&[("mix.exs", MIX_EXS), ("mix.lock", &text)]);
    let error = lock(dir.path()).expect_err("refused");
    assert!(
        error.to_string().contains(
            "mix.lock entry deep_merge is a git dependency; only hex entries are supported"
        ),
        "{error}"
    );
}

#[test]
fn a_private_repository_fails() {
    let text = MIX_LOCK.replace("[], \"hexpm\"", "[], \"acme\"");
    let dir = set(&[("mix.exs", MIX_EXS), ("mix.lock", &text)]);
    let error = lock(dir.path()).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("mix.lock entry deep_merge is not from the hexpm repository"),
        "{error}"
    );
}

#[test]
fn a_git_rebar_entry_fails() {
    let text = REBAR_LOCK.replace(
        "{pkg,<<\"jsx\">>,<<\"3.1.0\">>}",
        "{git,\"https://github.com/talentdeficit/jsx.git\",{ref,\"abc\"}}",
    );
    let dir = set(&[("rebar.config", REBAR_CONFIG), ("rebar.lock", &text)]);
    let error = lock(dir.path()).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("rebar.lock entry jsx is a git dependency; only pkg entries are supported"),
        "{error}"
    );
}

#[test]
fn a_rebar_entry_without_a_checksum_fails() {
    let text = REBAR_LOCK.replace(
        " {<<\"jsx\">>, <<\"0C5CC8FDC11B53CC25CF65AC6705AD39E54ECC56D1C22E4ADB8F5A53FB9427F3\">>}]}",
        "]}",
    );
    let text = text.replace("<<\"163B73F6367A7341B33C794C4E88E7DBFE6498AC42DCD69EF44C5BC5507C8DB0\">>},", "<<\"163B73F6367A7341B33C794C4E88E7DBFE6498AC42DCD69EF44C5BC5507C8DB0\">>}");
    let dir = set(&[("rebar.config", REBAR_CONFIG), ("rebar.lock", &text)]);
    let error = lock(dir.path()).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("rebar.lock has no pkg_hash_ext for jsx"),
        "{error}"
    );
}
