use std::path::PathBuf;

use super::host::{Host, Server, same_path};

pub const BUDGET_SECS: u64 = 20;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keep {
    pub workspaces: Vec<PathBuf>,
    pub roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reaped {
    pub servers: usize,
    pub bytes: u64,
}

impl Keep {
    fn allows(&self, server: &Server) -> bool {
        let (Some(workspace), Some(_), Some(root)) = (
            server.workspace.as_deref(),
            server.output_base.as_deref(),
            server.output_user_root.as_deref(),
        ) else {
            return false;
        };
        self.roots.iter().any(|keep| same_path(root, keep))
            && !self
                .workspaces
                .iter()
                .any(|keep| same_path(workspace, keep))
    }
}

pub fn reap_idle(host: &dyn Host, keep: &Keep, target: u64, min_idle: u64) -> Reaped {
    let start = host.now();
    let mut candidates: Vec<(u64, Server)> = host
        .servers()
        .into_iter()
        .filter(|server| keep.allows(server))
        .filter_map(|server| {
            let used = host.server_last_used(&server)?;
            (start.saturating_sub(used) >= min_idle && host.server_idle(&server) == Some(true))
                .then_some((used, server))
        })
        .collect();
    candidates.sort_by_key(|(used, server)| (*used, server.pid));
    let mut reaped = Reaped::default();
    for (_, server) in candidates {
        if reaped.bytes >= target
            || host.interrupted().is_some()
            || host.now().saturating_sub(start) >= BUDGET_SECS
        {
            break;
        }
        if host.server_idle(&server) != Some(true) {
            continue;
        }
        let bytes = host.server_footprint(&server);
        if host.shutdown_server(&server) {
            reaped.servers += 1;
            reaped.bytes = reaped.bytes.saturating_add(bytes);
        }
    }
    reaped
}
