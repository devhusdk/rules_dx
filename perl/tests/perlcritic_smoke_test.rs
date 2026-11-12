//! The perlcritic shim passes a clean file, reports RequireUseStrict on a dirty one and ignores a
//! .perlcriticrc in the working directory under --noprofile.

use std::path::Path;

use dx_testing::{Run, command_for, env_file, path_text, run_command, scratch_dir, write};

fn perlcritic(source: &str, cwd: &Path) -> Run {
    let source = path_text(&env_file(source));
    let mut command = command_for(&env_file("PERLCRITIC"), &[]).expect("command");
    command.current_dir(cwd).args([
        "--noprofile",
        "--severity=5",
        "--nocolor",
        "--quiet",
        "--verbose=%f\t%l\t%p\t%m\n",
        &source,
    ]);
    run_command(&mut command, std::time::Duration::from_secs(120)).expect("perlcritic runs")
}

#[test]
fn clean_file_passes() {
    let out = perlcritic("CLEAN", &scratch_dir("clean"));
    assert!(out.status.success(), "{}", out.combined());
}

#[test]
fn dirty_file_reports_require_use_strict() {
    let out = perlcritic("DIRTY", &scratch_dir("dirty"));
    assert_eq!(out.status.code(), Some(2), "{}", out.combined());
    assert!(
        out.stdout.contains("TestingAndDebugging::RequireUseStrict"),
        "{}",
        out.combined()
    );
}

#[test]
fn noprofile_ignores_working_directory_profile() {
    let work = scratch_dir("profile");
    write(
        &work.join(".perlcriticrc"),
        "[-TestingAndDebugging::RequireUseStrict]\n",
    );
    let out = perlcritic("DIRTY", &work);
    assert_eq!(out.status.code(), Some(2), "{}", out.combined());
    assert!(
        out.stdout.contains("TestingAndDebugging::RequireUseStrict"),
        "{}",
        out.combined()
    );
}
