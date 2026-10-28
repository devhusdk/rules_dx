#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdoptError {
    #[error("create parent {parent}: {detail}")]
    CreateParent { parent: String, detail: String },
    #[error("write {path}: {detail}")]
    WriteFile { path: String, detail: String },
    #[error("read {path}: {detail}")]
    ReadFile { path: String, detail: String },
    #[error("read version pin: {detail}")]
    ReadVersionPin { detail: String },
    #[error("read MODULE.bazel: {detail}")]
    ReadModuleFile { detail: String },
    #[error("MODULE.bazel or its includes: {detail}")]
    LoadModuleFile { detail: String },
    #[error("refuses empty version")]
    EmptyVersion,
    #[error("launcher version is not semver: {version:?}")]
    LauncherVersion { version: String },
    #[error("launcher sha256 pins: {detail}")]
    LauncherPins { detail: String },
    #[error("create .dx: {detail}")]
    CreateDxDir { detail: String },
    #[error("write version pin: {detail}")]
    WriteVersionPin { detail: String },
    #[error("dx watch refuses CI (local-only)")]
    WatchRefusesCi,
    #[error("not watchable: {command}")]
    NotWatchable { command: String },
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
    #[error("no qualified migration {from} -> {to}: manifest {manifest} is not shipped")]
    NoQualifiedManifest {
        from: String,
        to: String,
        manifest: String,
    },
    #[error("no recorded pin operation: nothing to roll back")]
    PinRecordMissing,
    #[error("unreadable pin record: {detail}")]
    PinRecordUnreadable { detail: String },
    #[error("write pin record: {detail}")]
    WritePinRecord { detail: String },
    #[error("invalid directory for dx init: {dir:?}: {reason}")]
    InitInvalidDir { dir: String, reason: String },
    #[error("invalid module name for dx init: {module:?}: {reason}")]
    InitInvalidModule { module: String, reason: String },
    #[error("init MODULE.bazel snippet: {detail}")]
    InitSnippet { detail: String },
    #[error(
        "unknown dx init part: {name} (want one of {})",
        crate::init_parts::INIT_PART_NAMES.join(", ")
    )]
    InitUnknownPart { name: String },
    #[error("read .dx/init.toml: {detail}")]
    InitRecord { detail: String },
    #[error("scaffold path escapes the workspace root: {path}")]
    ScaffoldEscapesRoot { path: String },
    #[error("{path} is not a directory")]
    NotADirectory { path: String },
    #[error("scan workspace for CUDA sources: {detail}")]
    WorkspaceScan { detail: String },
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
    use super::AdoptError;
    use crate::watch::plan_watch;

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
    fn scaffold_errors_render_stably() {
        assert_eq!(
            AdoptError::InitInvalidDir {
                dir: "../evil".to_owned(),
                reason: "directories stay inside the current directory (no empty, '.' or '..' segments)".to_owned(),
            }
            .to_string(),
            "invalid directory for dx init: \"../evil\": directories stay inside the current directory (no empty, '.' or '..' segments)"
        );
        assert_eq!(
            AdoptError::InitInvalidModule {
                module: "Bad".to_owned(),
                reason: "module names start with [a-z0-9] and use [a-z0-9._-] only".to_owned(),
            }
            .to_string(),
            "invalid module name for dx init: \"Bad\": module names start with [a-z0-9] and use [a-z0-9._-] only"
        );
        assert_eq!(
            AdoptError::ScaffoldEscapesRoot {
                path: "../evil".to_owned(),
            }
            .to_string(),
            "scaffold path escapes the workspace root: ../evil"
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
            AdoptError::UnknownShell {
                shell: "tcsh".to_owned()
            }
            .to_string(),
            "unknown-shell: tcsh"
        );
        assert_eq!(
            AdoptError::EmptyVersion.to_string(),
            "refuses empty version"
        );
        assert_eq!(
            AdoptError::ReadVersionPin {
                detail: "denied".to_owned()
            }
            .to_string(),
            "read version pin: denied"
        );
        assert_eq!(
            AdoptError::ReadModuleFile {
                detail: "denied".to_owned()
            }
            .to_string(),
            "read MODULE.bazel: denied"
        );
        assert_eq!(
            AdoptError::LoadModuleFile {
                detail: "bad".to_owned()
            }
            .to_string(),
            "MODULE.bazel or its includes: bad"
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
