use super::super::test_support::*;

#[test]
fn check_summary_names_the_plain_update_command() {
    assert_eq!(super::RunMode::Check.command_text(), "dx update");
}

#[test]
fn update_without_dependency_sets_reports_none() {
    let harness = Harness::new("update-no-sets");
    let (code, _, err) = harness.run(&["update"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no dependency sets"), "{err}");
}

#[test]
fn update_names_a_manifest_without_a_lock() {
    let harness = Harness::new("update-unlocked");
    harness.write_source("Cargo.toml", "[package]\nname = \"app\"\n");
    let (code, _, err) = harness.run(&["update"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("no lock yet for Cargo.toml; run dx generate --apply"),
        "{err}"
    );
}

#[test]
fn update_ignores_a_stray_dx_toml() {
    let harness = Harness::new("update-stray-dx-toml");
    harness.write_source("dx.toml", "[[dependency_set]]\nname = \"x\"\nnot toml {{");
    let (code, _, err) = harness.run(&["update"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no dependency sets"), "{err}");
}

#[test]
fn backend_error_codes_match_the_shared_constants() {
    use crate::exec::common::{
        CODE_BACKEND_UNAVAILABLE, CODE_FROZEN_LOCKED, CODE_OFFLINE_REQUIRED,
    };
    use dx_update::backend::BackendError;
    let set = "s".to_owned();
    let unavailable = BackendError::BackendUnavailable {
        set: set.clone(),
        tool: "uv",
        var: "DX_UV_BIN",
        what: "uv",
        detail: None,
    };
    assert_eq!(
        BackendError::OfflineRequired { set: set.clone() }.code(),
        CODE_OFFLINE_REQUIRED
    );
    assert_eq!(
        BackendError::FrozenLocked { set }.code(),
        CODE_FROZEN_LOCKED
    );
    assert_eq!(unavailable.code(), CODE_BACKEND_UNAVAILABLE);
}

#[test]
fn frozen_locked_code_is_stable_single_source() {
    assert_eq!(crate::exec::common::CODE_FROZEN_LOCKED, "frozen_locked");
}

#[test]
fn offline_required_code_is_stable_single_source() {
    assert_eq!(
        crate::exec::common::CODE_OFFLINE_REQUIRED,
        "offline_required"
    );
}

#[test]
fn backend_and_runner_statuses_keep_their_wording() {
    use dx_process::ChildStatus;
    use dx_update::backend::BackendError as E;
    use dx_update::outcome::SetStatus as S;

    let unsupported = "unsupported selective update for go: nope";
    let offline = "offline_required: cannot update go without network (re-run without --offline once connected)";
    let frozen = "frozen_locked: cannot change go resolution while frozen (re-run without --frozen to allow resolver changes)";
    let missing = "backend_unavailable: cannot run uv for go: set DX_UV_BIN to the absolute path of the pinned uv; PATH is never searched";
    let errors = vec![
        (
            E::Unsupported {
                set: "go".to_owned(),
                reason: "nope",
            },
            false,
            unsupported,
        ),
        (
            E::OfflineRequired {
                set: "go".to_owned(),
            },
            true,
            offline,
        ),
        (
            E::FrozenLocked {
                set: "go".to_owned(),
            },
            true,
            frozen,
        ),
        (
            E::BackendUnavailable {
                set: "go".to_owned(),
                tool: "uv",
                var: "DX_UV_BIN",
                what: "the pinned uv",
                detail: None,
            },
            true,
            missing,
        ),
    ];
    for (error, failed, text) in &errors {
        for verb in ["update", "check"] {
            let (status, message) = super::backend_error_status(error, verb, "go");
            let (want_status, lead) = if *failed {
                (S::Failed, "failed to")
            } else {
                (S::Unsupported, "cannot")
            };
            assert_eq!(status, want_status);
            assert_eq!(message, format!("{lead} {verb} go: {text}"));
        }
    }

    let ok = || (S::Updated, "ok".to_owned());
    let launch = Err(std::io::Error::other("boom"));
    assert_eq!(
        super::runner_status(launch, "check", "go", ok()),
        (
            S::Failed,
            "failed to check go: failed to launch updater: boom".to_owned()
        )
    );
    assert_eq!(
        super::runner_status(Ok(ChildStatus::exited(0)), "update", "go", ok()),
        ok()
    );
    assert_eq!(
        super::runner_status(Ok(ChildStatus::exited(3)), "update", "go", ok()),
        (
            S::Failed,
            "failed to update go: updater exited 3".to_owned()
        )
    );
    assert_eq!(
        super::runner_status(Ok(ChildStatus::signalled(None)), "check", "go", ok()),
        (
            S::Failed,
            "failed to check go: updater terminated by signal".to_owned()
        )
    );
}
