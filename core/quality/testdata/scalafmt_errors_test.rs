//! scalafmt exits 2 naming the file on a parse error and 8 naming the option on a config error.

use dx_testing::{env_file, path_text, run, scratch_dir, write};

#[test]
fn parse_and_config_errors_have_distinct_exit_codes() {
    let scalafmt = env_file("SCALAFMT");
    let good = path_text(&env_file("GOOD_CONFIG"));
    let bad = path_text(&env_file("BAD_CONFIG"));
    let work = scratch_dir("scalafmt_errors");
    let broken = work.join("Broken.scala");
    let fine = work.join("Fine.scala");
    write(&broken, "package fixtures.scalafmt\n\nobject Sample {\n  def greet = (\n}\n");
    write(&fine, "package fixtures.scalafmt\n\nobject Sample {\n  def greet = 1\n}\n");

    let parse = run(&scalafmt, &["--config", &good, "--list", &path_text(&broken)], &[]).expect("run scalafmt");
    assert!(
        parse.status.code() == Some(2) && parse.stderr.contains("Broken.scala:5: error"),
        "parse error exited {:?}, expected 2 naming Broken.scala\n{}",
        parse.status.code(),
        parse.stderr
    );

    let config = run(&scalafmt, &["--config", &bad, "--list", &path_text(&fine)], &[]).expect("run scalafmt");
    assert!(
        config.status.code() == Some(8) && config.stderr.contains("noSuchOption"),
        "config error exited {:?}, expected 8 naming noSuchOption\n{}",
        config.status.code(),
        config.stderr
    );
}
