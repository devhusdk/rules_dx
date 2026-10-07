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

pub fn policy_for(tool_id: &str) -> RunfilesPolicy {
    match tool_id {
        "eslint" | "prettier" => RunfilesPolicy::OwnManifest,
        _ => RunfilesPolicy::Inherit,
    }
}

impl ResolvedTool {
    pub fn direct(tool_id: &str, program: &Path) -> Self {
        Self {
            program: program.to_owned(),
            argv_prefix: Vec::new(),
            cwd_rel: String::new(),
            runfiles: RunfilesPolicy::Inherit,
            tool_id: tool_id.to_owned(),
        }
    }

    pub fn javascript(tool_id: &str, program: &Path) -> Self {
        Self {
            program: program.to_owned(),
            argv_prefix: Vec::new(),
            cwd_rel: String::new(),
            runfiles: RunfilesPolicy::OwnManifest,
            tool_id: tool_id.to_owned(),
        }
    }

    pub fn resolve(&self, args: &[OsString], files: &[&Path]) -> Result<Invocation, LaunchError> {
        if self.program.as_os_str().is_empty() {
            return Err(LaunchError::MissingProgram {
                tool: self.tool_id.clone(),
            });
        }
        let mut argv = Vec::with_capacity(1 + self.argv_prefix.len() + args.len() + files.len());
        argv.push(self.program.as_os_str().to_owned());
        argv.extend(self.argv_prefix.iter().cloned());
        argv.extend(args.iter().cloned());
        argv.extend(files.iter().map(|path| path.as_os_str().to_owned()));
        Ok(Invocation {
            argv,
            cwd_rel: self.cwd_rel.clone(),
        })
    }
}

#[path = "launch_tests.rs"]
#[cfg(test)]
mod launch_tests;
