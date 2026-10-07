#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdoptError {
    #[error("create parent {parent}: {detail}")]
    CreateParent { parent: String, detail: String },
    #[error("write {path}: {detail}")]
    WriteFile { path: String, detail: String },
    #[error("create hooks dir: {detail}")]
    CreateHooksDir { detail: String },
    #[error("read hook {trigger}: {detail}")]
    ReadHook { trigger: String, detail: String },
    #[error("unmanaged hook refuses install: {trigger}")]
    UnmanagedInstall { trigger: String },
    #[error("write hook {trigger}: {detail}")]
    WriteHook { trigger: String, detail: String },
    #[error("stat hook {trigger}: {detail}")]
    StatHook { trigger: String, detail: String },
    #[error("chmod hook {trigger}: {detail}")]
    ChmodHook { trigger: String, detail: String },
    #[error("write overlay: {detail}")]
    WriteOverlay { detail: String },
    #[error("render overlay: {detail}")]
    RenderOverlay { detail: String },
    #[error("unmanaged hook refuses uninstall: {trigger}")]
    UnmanagedUninstall { trigger: String },
    #[error("remove hook {trigger}: {detail}")]
    RemoveHook { trigger: String, detail: String },
    #[error("read version pin: {detail}")]
    ReadVersionPin { detail: String },
    #[error("refuses empty version")]
    EmptyVersion,
    #[error("create .dx: {detail}")]
    CreateDxDir { detail: String },
    #[error("write version pin: {detail}")]
    WriteVersionPin { detail: String },
    #[error("dx watch refuses CI (local-only)")]
    WatchRefusesCi,
    #[error("not watchable: {command}")]
    NotWatchable { command: String },
    #[error("rejected scope: {scope}")]
    RejectedScope { scope: String },
    #[error("dx why needs <file> <label>: resolve the file owner first, then plan_somepath")]
    WhyNeedsOwner,
    #[error("unknown inspect: {kind}")]
    UnknownInspect { kind: String },
    #[error("unknown-shell: {shell}")]
    UnknownShell { shell: String },
    #[error("watch spawn failed: {detail}")]
    WatchSpawn { detail: String },
    #[error("watch failed: {detail}")]
    WatchFailed { detail: String },
    #[error("migrate needs distinct versions: {detail}")]
    MigrateVersions { detail: String },
    #[error("migrate is upgrade-only: {from} -> {to}")]
    MigrateNotUpgrade { from: String, to: String },
    #[error("unknown language for dx new: {language} (want one of rust, python, javascript, typescript, go, java, kotlin, scala, csharp, fsharp, c, cc, cpp)")]
    NewUnknownLanguage { language: String },
    #[error("invalid hooks config: {detail}")]
    InvalidHooks { detail: String },
    #[error("invalid hooks timings: {detail}")]
    InvalidTimings { detail: String },
    #[error("render timings: {detail}")]
    RenderTimings { detail: String },
    #[error("invalid invocation defaults: {detail}")]
    InvalidDefaults { detail: String },
    /// A boolean environment default is neither a documented on nor a documented off.
    #[error("invalid invocation default {name}={value:?}: want one of {spellings}")]
    InvalidEnvBool {
        name: &'static str,
        value: String,
        spellings: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::super::{plan_watch, AdoptError as FacadeError};
    use super::AdoptError;

    #[test]
    fn error_reexports_match_local_definitions() {
        assert_eq!(FacadeError::EmptyVersion, AdoptError::EmptyVersion);
        assert_eq!(
            FacadeError::WatchRefusesCi.to_string(),
            AdoptError::WatchRefusesCi.to_string()
        );
    }

    #[test]
    fn watch_errors_render_stably() {
        assert_eq!(
            AdoptError::WatchSpawn {
                detail: "denied".to_owned()
            }
            .to_string(),
            "watch spawn failed: denied"
        );
        assert_eq!(
            AdoptError::WatchFailed {
                detail: "boom".to_owned()
            }
            .to_string(),
            "watch failed: boom"
        );
    }

    #[test]
    fn adopt_errors_render_byte_identical_to_legacy_strings() {
        assert_eq!(
            AdoptError::WatchRefusesCi.to_string(),
            "dx watch refuses CI (local-only)"
        );
        assert_eq!(
            AdoptError::NotWatchable {
                command: "docs".to_owned()
            }
            .to_string(),
            "not watchable: docs"
        );
        assert_eq!(
            AdoptError::RejectedScope {
                scope: "//a:one".to_owned()
            }
            .to_string(),
            "rejected scope: //a:one"
        );
        assert_eq!(
            AdoptError::UnknownShell {
                shell: "tcsh".to_owned()
            }
            .to_string(),
            "unknown-shell: tcsh"
        );
        assert_eq!(
            AdoptError::UnknownInspect {
                kind: "bogus".to_owned()
            }
            .to_string(),
            "unknown inspect: bogus"
        );
        assert_eq!(
            AdoptError::WhyNeedsOwner.to_string(),
            "dx why needs <file> <label>: resolve the file owner first, then plan_somepath"
        );
        assert_eq!(
            AdoptError::EmptyVersion.to_string(),
            "refuses empty version"
        );
        assert_eq!(
            AdoptError::UnmanagedInstall {
                trigger: "pre-commit".to_owned()
            }
            .to_string(),
            "unmanaged hook refuses install: pre-commit"
        );
        assert_eq!(
            AdoptError::ReadVersionPin {
                detail: "denied".to_owned()
            }
            .to_string(),
            "read version pin: denied"
        );
        assert_eq!(
            plan_watch("docs", false).unwrap_err().to_string(),
            "not watchable: docs"
        );
        assert_eq!(
            AdoptError::UnknownShell {
                shell: "tcsh".to_owned()
            }
            .to_string(),
            "unknown-shell: tcsh"
        );
    }
}
