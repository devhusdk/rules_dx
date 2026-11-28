use dx_plan::args::Command;
use dx_plan::resolve::ResolvedScope;

fn labels(targets: &[&str]) -> ResolvedScope {
    let targets: Vec<String> = targets.iter().map(|target| (*target).to_owned()).collect();
    ResolvedScope {
        scope: dx_process::Scope::Labels(targets.clone()),
        targets,
        excluded: Vec::new(),
        kept: Vec::new(),
    }
}

fn member(command: Command, aspects: &[&str], resolved: ResolvedScope) -> super::Member {
    super::Member {
        command,
        aspects: aspects.iter().map(|name| (*name).to_owned()).collect(),
        resolved,
    }
}

fn shared_build(
    scopes: [ResolvedScope; 2],
    failing: Option<&[(Option<&str>, Option<&str>)]>,
) -> super::SharedBuild {
    let [format, lint] = scopes;
    super::SharedBuild {
        bep: std::path::PathBuf::from("unused.json"),
        events: None,
        members: vec![
            member(Command::Format, &["q:f.bzl%format", "q:f.bzl%both"], format),
            member(Command::Lint, &["q:f.bzl%lint", "q:f.bzl%both"], lint),
        ],
        bazel_options: Vec::new(),
        code: 1,
        failing: failing.map(|failing| {
            failing
                .iter()
                .map(|(label, aspect)| (label.map(str::to_owned), aspect.map(str::to_owned)))
                .collect()
        }),
        unread: 2,
    }
}

fn failed_build(failing: Option<&[Option<&str>]>) -> super::SharedBuild {
    let failing: Option<Vec<(Option<&str>, Option<&str>)>> = failing.map(|failing| {
        failing
            .iter()
            .map(|aspect| (Some("//a:x"), *aspect))
            .collect()
    });
    shared_build([labels(&["//..."]), labels(&["//..."])], failing.as_deref())
}

#[test]
fn a_failure_goes_to_every_phase_that_passed_its_aspect() {
    let codes = |failing: Option<&[Option<&str>]>| {
        let build = failed_build(failing);
        (
            build.phase_code(Command::Format),
            build.phase_code(Command::Lint),
        )
    };
    assert_eq!(codes(Some(&[Some("q:f.bzl%lint")])), (0, 1));
    assert_eq!(codes(Some(&[Some("q:f.bzl%format")])), (1, 0));
    assert_eq!(
        codes(Some(&[Some("q:f.bzl%both")])),
        (1, 1),
        "shared aspect"
    );
    assert_eq!(
        codes(Some(&[Some("q:f.bzl%lint"), Some("q:f.bzl%format")])),
        (1, 1),
        "a foreign failure masks no own failure"
    );
    assert_eq!(codes(Some(&[None])), (1, 1), "target-level failure");
    assert_eq!(
        codes(Some(&[Some("q:f.bzl%other")])),
        (1, 1),
        "unknown aspect"
    );
    assert_eq!(codes(None), (1, 1), "unreadable events");
}

#[test]
fn a_failure_goes_only_to_the_phases_that_own_its_target() {
    let codes = |failing: &[(Option<&str>, Option<&str>)]| {
        let build = shared_build(
            [labels(&["//a:f"]), labels(&["//a:f", "//a:l"])],
            Some(failing),
        );
        (
            build.phase_code(Command::Format),
            build.phase_code(Command::Lint),
        )
    };
    assert_eq!(codes(&[(Some("//a:l"), Some("q:f.bzl%format"))]), (0, 0));
    assert_eq!(codes(&[(Some("//a:l"), Some("q:f.bzl%both"))]), (0, 1));
    assert_eq!(codes(&[(Some("@@//a:l"), None)]), (0, 1), "target-level");
    assert_eq!(codes(&[(Some("//a:f"), None)]), (1, 1), "shared target");
    assert_eq!(codes(&[(Some("//a:f"), Some("q:f.bzl%lint"))]), (0, 1));
    assert_eq!(codes(&[(None, Some("q:f.bzl%format"))]), (1, 0), "no label");
    assert_eq!(
        codes(&[(Some("//other:t"), None)]),
        (1, 1),
        "unlisted label"
    );
}

#[test]
fn each_phase_leaves_out_the_targets_only_other_phases_selected() {
    let build = shared_build(
        [labels(&["//a:f", "//a:g"]), labels(&["//a:f", "//a:l"])],
        None,
    );
    let format = build.foreign(Command::Format);
    assert_eq!(format.labels, ["//a:l".to_owned()].into());
    assert_eq!(format.aspects, ["q:f.bzl%lint".to_owned()].into());
    let lint = build.foreign(Command::Lint);
    assert_eq!(lint.labels, ["//a:g".to_owned()].into());
    let same = shared_build([labels(&["//..."]), labels(&["//..."])], None);
    assert!(same.foreign(Command::Format).labels.is_empty());
}

#[test]
fn target_lists_share_a_build_and_patterns_only_an_identical_one() {
    let format = labels(&["//a:f"]);
    let lint = labels(&["//a:f", "//b:l"]);
    let everything = labels(&["//..."]);
    assert!(super::shares_build(&lint, &format));
    assert!(super::shares_build(&everything, &everything));
    assert!(!super::shares_build(&everything, &format));
    assert!(!super::shares_build(&labels(&["//b:all"]), &format));
    let union = super::union_scope(&[(Command::Format, format), (Command::Lint, lint.clone())])
        .expect("union");
    assert_eq!(union.targets, lint.targets);
    let same = super::union_scope(&[
        (Command::Format, everything.clone()),
        (Command::Lint, everything.clone()),
    ]);
    assert_eq!(same, Some(everything));
}
