use super::run::*;
use std::path::Path;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

#[test]
fn options_default_to_sixty_seconds() {
    let options = parse_options(&[]).expect("options");
    assert_eq!(options.seconds, "60");
    assert_eq!(options.out, None);
}

#[test]
fn options_read_every_flag() {
    let options = parse_options(&strings(&[
        "--time", "5", "--out", "o", "--jobs", "2", "--repro", "r",
    ]))
    .expect("options");
    assert_eq!(options.seconds, "5");
    assert_eq!(options.out.as_deref(), Some("o"));
    assert_eq!(options.jobs.as_deref(), Some("2"));
    assert_eq!(options.repro.as_deref(), Some("r"));
}

#[test]
fn an_unknown_flag_or_missing_value_is_a_usage_error() {
    assert_eq!(parse_options(&strings(&["--bogus"])), Err(USAGE.to_owned()));
    assert_eq!(parse_options(&strings(&["--time"])), Err(USAGE.to_owned()));
}

#[test]
fn libfuzzer_forks_when_jobs_are_given() {
    let options = parse_options(&strings(&["--jobs", "3"])).expect("options");
    let args = fuzz_args(
        Engine::Libfuzzer,
        "",
        &options,
        Path::new("/w"),
        Path::new("/w/artifacts"),
    );
    assert_eq!(args[0], "-max_total_time=60");
    assert_eq!(args[1], "-artifact_prefix=/w/artifacts/");
    assert_eq!(args[2], "-fork=3");
}

#[test]
fn go_fuzzes_the_named_target() {
    let options = parse_options(&strings(&["--time", "4"])).expect("options");
    let args = fuzz_args(
        Engine::Go,
        "FuzzX",
        &options,
        Path::new("/w"),
        Path::new("/c"),
    );
    assert_eq!(args[0], "-test.fuzz=^FuzzX$");
    assert_eq!(args[2], "-test.fuzztime=4s");
}

#[test]
fn repro_replays_the_file() {
    assert_eq!(
        repro_args(Engine::Jazzer, "a.B", Path::new("/r")),
        strings(&["--target_class=a.B", "/r"])
    );
    assert_eq!(
        repro_args(Engine::Go, "FuzzX", Path::new("/r")),
        strings(&["-test.run=^FuzzX$"])
    );
}
