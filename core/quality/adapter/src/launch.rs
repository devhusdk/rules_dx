//! Explicit launch descriptions for quality tools.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::commands::Invocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunfilesPolicy {
    Inherit,
    OwnManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTool {
    pub program: PathBuf,
    pub argv_prefix: Vec<OsString>,
    pub cwd_rel: String,
    pub runfiles: RunfilesPolicy,
    pub tool_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("tool {tool:?} names no executable: declare the wrapper binary before launch")]
    MissingProgram { tool: String },
}

#[must_use]
pub fn policy_for(tool_id: &str) -> RunfilesPolicy {
    match tool_id {
        "asciidoctor" | "eslint" | "gplint" | "graphql_eslint" | "prettier" | "redocly"
        | "remark" | "stylelint" => RunfilesPolicy::OwnManifest,
        _ => RunfilesPolicy::Inherit,
    }
}

impl ResolvedTool {
    #[must_use]
    pub fn direct(tool_id: &str, program: &Path) -> Self {
        Self {
            program: program.to_owned(),
            argv_prefix: Vec::new(),
            cwd_rel: String::new(),
            runfiles: RunfilesPolicy::Inherit,
            tool_id: tool_id.to_owned(),
        }
    }

    #[must_use]
    pub fn javascript(tool_id: &str, program: &Path) -> Self {
        Self {
            program: program.to_owned(),
            argv_prefix: Vec::new(),
            cwd_rel: String::new(),
            runfiles: RunfilesPolicy::OwnManifest,
            tool_id: tool_id.to_owned(),
        }
    }

    /// # Errors
    ///
    /// Returns [`LaunchError`] when the tool cannot be launched.
    pub fn resolve(&self, args: &[OsString], files: &[&Path]) -> Result<Invocation, LaunchError> {
        if self.program.as_os_str().is_empty() {
            return Err(LaunchError::MissingProgram {
                tool: self.tool_id.clone(),
            });
        }
        let mut command_line =
            Vec::with_capacity(1 + self.argv_prefix.len() + args.len() + files.len());
        command_line.push(self.program.as_os_str().to_owned());
        command_line.extend(self.argv_prefix.iter().cloned());
        command_line.extend(args.iter().cloned());
        command_line.extend(files.iter().map(|path| path.as_os_str().to_owned()));
        Ok(Invocation {
            argv: command_line,
            cwd_rel: self.cwd_rel.clone(),
        })
    }
}

#[path = "launch_tests.rs"]
#[cfg(test)]
mod launch_tests;
