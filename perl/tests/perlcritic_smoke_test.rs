//! The perlcritic wrapper under the strict tier profile dx binds.

use std::path::Path;

use dx_testing::{Run, command_for, env_file, path_text, run_command, scratch_dir, write};

fn perlcritic(source: &str, cwd: &Path) -> Run {
    let source = path_text(&env_file(source));
    let profile = path_text(&env_file("STRICT_PROFILE"));
    let mut command = command_for(&env_file("PERLCRITIC"), &[]).expect("command");
    command.current_dir(cwd).args([
        "--profile",
        &profile,
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
fn bound_profile_ignores_working_directory_profile() {
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

fn strict_findings(name: &str, text: &str) -> Run {
    let work = scratch_dir(name);
    let source = work.join(name);
    write(&source, text);
    let profile = path_text(&env_file("STRICT_PROFILE"));
    let mut command = command_for(&env_file("PERLCRITIC"), &[]).expect("command");
    command.current_dir(&work).args([
        "--profile",
        &profile,
        "--nocolor",
        "--quiet",
        "--verbose=%l\t%p\n",
        &path_text(&source),
    ]);
    run_command(&mut command, std::time::Duration::from_secs(120)).expect("perlcritic runs")
}

const GUARDED: &str = "package Guard;
use strict;
use warnings;

sub run {
    my ($path) = @_;
    eval q{1};
    open my $fh, q{<}, $path;
    open FH, $path or die;
    unlink $path;
    {
        no warnings;
        ## no critic
        my $x = 1;
    }
    {
        no warnings q{once};
    }
    return undef;
}

1;
";

#[test]
fn strict_profile_reports_each_guard_policy() {
    let out = strict_findings("Guard.pm", GUARDED);
    assert_eq!(out.status.code(), Some(2), "{}", out.combined());
    for expected in [
        "7\tBuiltinFunctions::ProhibitStringyEval",
        "8\tInputOutput::RequireCheckedOpen",
        "9\tInputOutput::ProhibitBarewordFileHandles",
        "9\tInputOutput::ProhibitTwoArgOpen",
        "10\tInputOutput::RequireCheckedSyscalls",
        "12\tTestingAndDebugging::ProhibitNoWarnings",
        "13\tMiscellanea::ProhibitUnrestrictedNoCritic",
        "19\tSubroutines::ProhibitExplicitReturnUndef",
    ] {
        assert!(
            out.stdout.contains(expected),
            "{expected}: {}",
            out.combined()
        );
    }
    assert!(
        !out.stdout
            .contains("17\tTestingAndDebugging::ProhibitNoWarnings"),
        "a category-restricted no warnings is allowed: {}",
        out.combined()
    );
}

#[test]
fn strict_profile_requires_strict_and_warnings() {
    let out = strict_findings("script.pl", "print q{hi};\n");
    assert_eq!(out.status.code(), Some(2), "{}", out.combined());
    for expected in [
        "TestingAndDebugging::RequireUseStrict",
        "TestingAndDebugging::RequireUseWarnings",
    ] {
        assert!(
            out.stdout.contains(expected),
            "{expected}: {}",
            out.combined()
        );
    }
}

#[test]
fn strict_profile_passes_a_clean_module() {
    let out = strict_findings(
        "Clean.pm",
        "package Clean;\nuse strict;\nuse warnings;\n\nour $VERSION = q{1.0};\n\nsub double {\n    my ($value) = @_;\n    return $value * 2;\n}\n\n1;\n",
    );
    assert!(out.status.success(), "{}", out.combined());
}
