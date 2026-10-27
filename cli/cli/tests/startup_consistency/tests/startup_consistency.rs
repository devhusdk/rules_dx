//! Every dx planner starts Bazel with the same startup flags under a local rc policy.

use dx_plan::args::{Command, Profile, WorkflowVerb};
use dx_plan::plan::{
    plan_build, plan_deploy_build, plan_deploy_run, plan_generate, plan_managed_with_roots,
    plan_run_build, plan_run_targets, plan_workflow,
};
use dx_plan::resolve::ResolvedScope;
use dx_process::{RcPolicy, Scope};

const OUTPUT_BASE: &str = "--output_base=/ob";
const DISK_CACHE: &str = "--disk_cache=/c/dx/disk-cache";

fn local() {
    dx_process::configure(RcPolicy {
        read_user_rc: true,
        disk_cache: Some("/c/dx/disk-cache".to_owned()),
        bazel_disk_cache: Some(std::path::PathBuf::from("/c/dx/disk-cache")),
        local_cpu: true,
        local_memory: true,
        disk_cache_gc: true,
        gc_idle_delay_secs: None,
    });
}

fn startup() -> Vec<String> {
    vec![OUTPUT_BASE.to_owned()]
}

fn resolved() -> ResolvedScope {
    ResolvedScope {
        scope: Scope::Labels(vec!["//a:x".to_owned()]),
        targets: vec!["//a:x".to_owned()],
    }
}

fn assert_prefix(argv: &[String], verb: &str) {
    assert_eq!(argv[..3], ["bazel", OUTPUT_BASE, verb], "{argv:?}");
    assert_eq!(argv[3], DISK_CACHE, "{argv:?}");
    for flag in ["--nohome_rc", "--nosystem_rc"] {
        assert!(!argv.iter().any(|word| word == flag), "{flag} in {argv:?}");
    }
}

#[test]
fn workflow_verbs_share_the_startup_prefix() {
    local();
    for verb in [
        WorkflowVerb::Build,
        WorkflowVerb::Test,
        WorkflowVerb::Coverage,
        WorkflowVerb::Run,
    ] {
        let plan = plan_workflow(
            verb,
            &resolved(),
            &[],
            Some("/tmp/bep.json"),
            None,
            &startup(),
        )
        .expect("plan");
        assert_prefix(&plan.argv, verb.name());
    }
}

#[test]
fn quality_builds_share_the_startup_prefix() {
    local();
    for command in [Command::Lint, Command::Typecheck, Command::Format] {
        let plan =
            plan_build(command, &resolved(), &[], "/tmp/bep.json", &startup()).expect("plan");
        assert_prefix(&plan.argv, "build");
    }
}

#[test]
fn generate_shares_the_startup_prefix() {
    local();
    for check in [false, true] {
        let plan = plan_generate(&resolved(), &[], check, &startup(), None).expect("plan");
        assert_prefix(&plan.argv, "run");
    }
}

#[test]
fn managed_builds_share_the_startup_prefix() {
    local();
    for command in [Command::Codegen, Command::Env] {
        let plan = plan_managed_with_roots(
            command,
            &["//a:x".to_owned()],
            &[],
            "/tmp/bep.json",
            &startup(),
        )
        .expect("plan");
        assert_prefix(&plan.argv, "build");
    }
}

#[test]
fn run_and_deploy_share_the_startup_prefix() {
    local();
    let targets = vec!["//a:x".to_owned()];
    let run = plan_run_targets(&targets, &[], &[], Profile::Dev, &startup()).expect("plan");
    assert_prefix(&run.argv, "run");
    let build = plan_run_build(&targets, &[], Profile::Dev, &startup()).expect("plan");
    assert_prefix(&build.argv, "build");
    let deploy_build = plan_deploy_build("//a:x", Profile::Release, &startup()).expect("plan");
    assert_prefix(&deploy_build.argv, "build");
    let deploy_run = plan_deploy_run("//a:x", &[], Profile::Release, &startup()).expect("plan");
    assert_prefix(&deploy_run.argv, "run");
}

#[test]
#[allow(clippy::disallowed_methods)] // test asserts the launcher head
fn query_and_shutdown_share_the_startup_prefix() {
    local();
    assert_eq!(dx_process::startup_argv(&startup()), ["bazel", OUTPUT_BASE]);
    assert!(dx_process::current().startup_opts().is_empty());
}
