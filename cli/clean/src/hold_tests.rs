use super::*;
use crate::apply::apply_plan;
use crate::fixtures::*;
use crate::inventory::collect_inventory;
use dx_atomic_fs::lease::GenerationUse;
use dx_setup::{setup_hex, GenerationId};
use std::time::Instant;

const WAIT: Duration = Duration::from_secs(10);
const SENTINEL: &str = "sentinel";

fn workspace(prefix: &str) -> (dx_test_scratch::TempDir, PathBuf) {
    let scratch = dx_test_scratch::scratch(prefix);
    let workspace = workspace_of(scratch.path());
    fs::create_dir_all(&workspace).expect("create workspace");
    (scratch, workspace)
}

fn materialize(workspace: &Path, pair: &SetupPair) {
    let dx_dir = workspace.join(DX_DIR_NAME);
    for (kind, hex) in [
        (GenerationUse::Environment, pair.environment.as_str()),
        (GenerationUse::Generated, pair.generated.as_str()),
    ] {
        let dir = dx_dir.join(kind.dir_name()).join(hex);
        fs::create_dir_all(&dir).expect("create generation dir");
        fs::write(dir.join(SENTINEL), hex).expect("write sentinel");
    }
}

fn select(workspace: &Path, pair: &SetupPair) {
    materialize(workspace, pair);
    dx_setup::commit_pair(workspace, pair).expect("commit");
}

fn generation_dir(workspace: &Path, kind: GenerationUse, hex: &str) -> PathBuf {
    workspace.join(DX_DIR_NAME).join(kind.dir_name()).join(hex)
}

fn exclusive_free(workspace: &Path, kind: GenerationUse, hex: &str) -> bool {
    lease::try_acquire_exclusive(&workspace.join(DX_DIR_NAME), kind, hex)
        .expect("probe lease")
        .is_some()
}

fn prune(workspace: &Path) -> crate::CleanOutcome {
    let plan = collect_inventory(workspace, &[], &[])
        .expect("collect")
        .plan();
    apply_plan(workspace, &plan).expect("apply")
}

#[test]
fn hold_current_leases_both_sides() {
    let (_scratch, workspace) = workspace("dx-clean-hold-both-");
    let pair = setup_pair('1', '2');
    select(&workspace, &pair);
    let held = hold_current(&workspace, WAIT)
        .expect("hold")
        .expect("a selection");
    assert_eq!(held.pair(), &pair);
    assert!(!exclusive_free(
        &workspace,
        GenerationUse::Environment,
        &digest('1')
    ));
    assert!(!exclusive_free(
        &workspace,
        GenerationUse::Generated,
        &digest('2')
    ));
    assert_eq!(
        held.lease_env(),
        format!("environments:{},generated:{}", digest('1'), digest('2'))
    );
    let files = held.into_files();
    assert_eq!(files.len(), 2);
    assert!(!exclusive_free(
        &workspace,
        GenerationUse::Environment,
        &digest('1')
    ));
    drop(files);
    assert!(exclusive_free(
        &workspace,
        GenerationUse::Environment,
        &digest('1')
    ));
    assert!(exclusive_free(
        &workspace,
        GenerationUse::Generated,
        &digest('2')
    ));
}

#[test]
fn hold_current_without_selection_is_none() {
    let (_scratch, workspace) = workspace("dx-clean-hold-none-");
    assert!(hold_current(&workspace, WAIT).expect("hold").is_none());
    assert!(
        !workspace.join(DX_DIR_NAME).exists(),
        "holding nothing creates nothing"
    );
    fs::create_dir_all(workspace.join(DX_DIR_NAME)).expect("create .dx");
    assert!(hold_current(&workspace, WAIT).expect("hold").is_none());
    assert!(matches!(
        hold_current(&workspace.join("missing"), WAIT),
        Err(HoldError::Clean(CleanError::WorkspaceRoot { .. }))
    ));
}

#[cfg(windows)]
fn link_dir(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).expect("link");
}

#[cfg(not(windows))]
fn link_dir(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("link");
}

#[test]
fn hold_current_waits_for_commit_lock_then_reads_new_current() {
    let (_scratch, workspace) = workspace("dx-clean-hold-wait-");
    let first = setup_pair('1', '2');
    let second = setup_pair('3', '4');
    select(&workspace, &second);
    let current = workspace.join(DX_DIR_NAME).join("setups").join("current");
    let second_target = fs::read_link(&current).expect("read current");
    select(&workspace, &first);
    let dx_dir = workspace.join(DX_DIR_NAME);
    let lock = acquire_lock(&dx_dir, WAIT).expect("hold the commit lock");
    let reader_workspace = workspace.clone();
    let reader = std::thread::spawn(move || {
        hold_current(&reader_workspace, WAIT)
            .expect("hold")
            .expect("a selection")
            .pair()
            .clone()
    });
    std::thread::sleep(Duration::from_millis(300));
    assert!(!reader.is_finished(), "the hold waits for the commit lock");
    dx_test_scratch::remove_directory_link(&current).expect("unlink current");
    link_dir(&second_target, &current);
    drop(lock);
    assert_eq!(reader.join().expect("reader"), second);
}

#[test]
fn hold_current_refuses_missing_or_foreign_generation_dir() {
    let (_scratch, workspace) = workspace("dx-clean-hold-foreign-");
    let pair = setup_pair('1', '2');
    dx_setup::commit_pair(&workspace, &pair).expect("commit");
    let env_dir = generation_dir(&workspace, GenerationUse::Environment, &digest('1'));
    let gen_dir = generation_dir(&workspace, GenerationUse::Generated, &digest('2'));
    let refused = |what: &str| match hold_current(&workspace, WAIT) {
        Err(HoldError::Generation { .. }) => {}
        other => panic!("{what}: want a refused generation, got {other:?}"),
    };
    refused("missing");
    assert!(!env_dir.exists(), "a missing generation is never created");
    assert!(!gen_dir.exists(), "a missing generation is never created");
    fs::create_dir_all(&env_dir).expect("create env dir");
    fs::create_dir_all(gen_dir.parent().expect("parent")).expect("create generated root");
    fs::write(&gen_dir, b"file").expect("file generation");
    refused("file");
    fs::remove_file(&gen_dir).expect("remove file");
    let elsewhere = workspace.join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("create link target");
    link_dir(&elsewhere, &gen_dir);
    refused("link");
    assert!(
        exclusive_free(&workspace, GenerationUse::Environment, &digest('1')),
        "a refused hold releases every lease"
    );
}

#[test]
fn hold_current_fails_closed_while_cleanup_holds_exclusive() {
    let (_scratch, workspace) = workspace("dx-clean-hold-removing-");
    select(&workspace, &setup_pair('1', '2'));
    let _exclusive = lease::try_acquire_exclusive(
        &workspace.join(DX_DIR_NAME),
        GenerationUse::Generated,
        &digest('2'),
    )
    .expect("probe")
    .expect("exclusive");
    let started = Instant::now();
    assert!(matches!(
        hold_current(&workspace, WAIT),
        Err(HoldError::Removing {
            kind: "generated",
            ..
        })
    ));
    assert!(started.elapsed() < WAIT, "the hold never waits on a lease");
    assert!(exclusive_free(
        &workspace,
        GenerationUse::Environment,
        &digest('1')
    ));
}

#[test]
fn apply_preserves_held_generation_after_selection_change() {
    let (_scratch, workspace) = workspace("dx-clean-hold-apply-");
    select(&workspace, &setup_pair('1', '2'));
    let held = hold_current(&workspace, WAIT)
        .expect("hold")
        .expect("a selection");
    select(&workspace, &setup_pair('3', '4'));
    let outcome = prune(&workspace);
    let mut skipped: Vec<(GenerationUse, String)> = outcome
        .skipped_leased_generations
        .iter()
        .map(|view| (view.kind, view.hex.clone()))
        .collect();
    skipped.sort_by_key(|(kind, _)| kind.dir_name());
    assert_eq!(
        skipped,
        vec![
            (GenerationUse::Environment, digest('1')),
            (GenerationUse::Generated, digest('2')),
        ]
    );
    let old_env = generation_dir(&workspace, GenerationUse::Environment, &digest('1'));
    assert_eq!(
        fs::read_to_string(old_env.join(SENTINEL)).expect("held bytes stay readable"),
        digest('1')
    );
    drop(held);
    let outcome = prune(&workspace);
    assert_eq!(outcome.removed_generations.len(), 2, "{outcome:?}");
    assert!(!old_env.exists());
}

fn iteration_pair(i: u64) -> SetupPair {
    SetupPair {
        environment: GenerationId::new(&format!("{:064x}", 2 * i + 1)).expect("env id"),
        generated: GenerationId::new(&format!("{:064x}", 2 * i + 2)).expect("gen id"),
    }
}

#[test]
fn hold_vs_apply_race_never_sees_partial_generation() {
    const ROUNDS: u64 = 25;
    let (_scratch, workspace) = workspace("dx-clean-hold-race-");
    select(&workspace, &iteration_pair(0));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
        let workspace = workspace.clone();
        let done = done.clone();
        std::thread::spawn(move || {
            let mut holds = 0usize;
            while !done.load(std::sync::atomic::Ordering::SeqCst) {
                let held = hold_current(&workspace, WAIT)
                    .expect("hold")
                    .expect("a selection");
                for (kind, hex) in [
                    (GenerationUse::Environment, held.pair().environment.as_str()),
                    (GenerationUse::Generated, held.pair().generated.as_str()),
                ] {
                    let sentinel = generation_dir(&workspace, kind, hex).join(SENTINEL);
                    std::thread::sleep(Duration::from_millis(2));
                    assert_eq!(
                        fs::read_to_string(&sentinel).expect("held generation is whole"),
                        hex
                    );
                }
                holds += 1;
            }
            holds
        })
    };
    for round in 1..=ROUNDS {
        select(&workspace, &iteration_pair(round));
        let _ = prune(&workspace);
    }
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(reader.join().expect("reader") > 0);
    let outcome = prune(&workspace);
    assert!(outcome.skipped_leased_generations.is_empty(), "{outcome:?}");
    let last = iteration_pair(ROUNDS);
    for (kind, keep) in [
        (GenerationUse::Environment, last.environment.as_str()),
        (GenerationUse::Generated, last.generated.as_str()),
    ] {
        let names: Vec<String> = fs::read_dir(workspace.join(DX_DIR_NAME).join(kind.dir_name()))
            .expect("list generations")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            vec![keep.to_owned()],
            "no pruned generation came back"
        );
    }
    assert_eq!(
        dx_setup::read_current_pair(&workspace)
            .expect("read")
            .map(|pair| setup_hex(&pair)),
        Some(setup_hex(&last))
    );
}

#[test]
fn a_streamed_child_keeps_its_selection_through_cleanup() {
    use dx_process::lifecycle::{
        run_streamed, CleanupBudget, EnvPolicy, Exit, StreamGroup, StreamOutcome, StreamOutput,
        StreamSpec, StreamStdin,
    };
    let (scratch, workspace) = workspace("dx-clean-hold-streamed-");
    select(&workspace, &setup_pair('1', '2'));
    let held = hold_current(&workspace, WAIT)
        .expect("hold")
        .expect("a selection");
    let alive = scratch.path().join("child.alive");
    let spec = StreamSpec {
        argv: vec![
            dx_testing::process_probe().into_os_string(),
            "--sleep-ms=2000".into(),
            format!("--alive-lock={}", alive.display()).into(),
        ],
        cwd: workspace.clone(),
        env: EnvPolicy::Inherited {
            extra: vec![(lease::LEASE_ENV_VAR.into(), held.lease_env().into())],
        },
        stdin: StreamStdin::Null,
        stdout: StreamOutput::Inherit,
        stderr: StreamOutput::Inherit,
        group: StreamGroup::Owned,
        held: held.into_files(),
    };
    let run = std::thread::spawn(move || run_streamed(spec, CleanupBudget::STANDARD, &|| None));
    let deadline = Instant::now() + WAIT;
    while !(alive.exists() && dx_testing::alive_lock_held(&alive)) {
        assert!(Instant::now() < deadline, "the probe never started");
        std::thread::sleep(Duration::from_millis(10));
    }
    select(&workspace, &setup_pair('3', '4'));
    let outcome = prune(&workspace);
    assert_eq!(outcome.skipped_leased_generations.len(), 2, "{outcome:?}");
    let old_gen = generation_dir(&workspace, GenerationUse::Generated, &digest('2'));
    assert!(
        old_gen.join(SENTINEL).is_file(),
        "the child's generation survives"
    );
    assert_eq!(
        run.join().expect("run thread").expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    let deadline = Instant::now() + WAIT;
    loop {
        let outcome = prune(&workspace);
        if outcome.skipped_leased_generations.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the lease outlived the child tree"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !old_gen.exists(),
        "cleanup removes the generation once the child is gone"
    );
}
