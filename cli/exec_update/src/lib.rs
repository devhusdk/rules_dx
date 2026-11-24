mod dep_edit;
mod staged;
#[cfg(test)]
mod test_env;
mod update;
mod update_add_missing;
mod update_advisory;
mod update_outdated;
mod update_pr;
mod update_prune;
mod update_unused;
mod update_version;
mod update_version_add;

use dx_exec_core::common::{Dispatch, Env};
use dx_plan::args::{Command, Invocation};

pub use update_outdated::registry_fetch;
pub use update_version::COMMIT_JSON_ENV;

pub fn execute_update(invocation: &Invocation, env: Env<'_>, dispatch: Dispatch<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Update,
        "update dispatch guards commands"
    );
    if update_advisory::selected(invocation) {
        update_advisory::execute_update_advisory(invocation, env, dispatch)
    } else if invocation.pr {
        update_pr::execute_update_pr(invocation, env, dispatch)
    } else if invocation.outdated {
        update_outdated::execute_update_outdated(invocation, env)
    } else if invocation.edits_version() {
        update_version::execute_update_version(invocation, env)
    } else {
        update::execute_update(invocation, env)
    }
}
