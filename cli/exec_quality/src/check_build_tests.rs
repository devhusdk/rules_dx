use dx_plan::args::Command;

/// A shared build of format and lint that failed with `failing`, lint passing one aspect format
/// also passes.
fn failed_build(failing: Option<&[Option<&str>]>) -> super::SharedBuild {
    let aspects = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
    super::SharedBuild {
        bep: std::path::PathBuf::from("unused.json"),
        members: vec![
            (
                Command::Format,
                aspects(&["q:f.bzl%format", "q:f.bzl%both"]),
            ),
            (Command::Lint, aspects(&["q:f.bzl%lint", "q:f.bzl%both"])),
        ],
        bazel_options: Vec::new(),
        resolved: dx_plan::resolve::ResolvedScope {
            scope: dx_process::Scope::Labels(Vec::new()),
            targets: Vec::new(),
            excluded: Vec::new(),
            kept: Vec::new(),
        },
        code: 1,
        failing: failing.map(|failing| {
            failing
                .iter()
                .map(|aspect| aspect.map(str::to_owned))
                .collect()
        }),
        unread: 2,
    }
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
    // Failures the events cannot pin on one member belong to every member, so none exits 0.
    assert_eq!(codes(Some(&[None])), (1, 1), "target-level failure");
    assert_eq!(
        codes(Some(&[Some("q:f.bzl%other")])),
        (1, 1),
        "unknown aspect"
    );
    assert_eq!(codes(None), (1, 1), "unreadable events");
}
