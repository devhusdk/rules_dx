//! Explains why actions in a newer execution log missed the cache.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::proto::exec_log_entry::Spawn;
use crate::{ExeclogError, InputMap, Log, Segment};

/// Changes kept per miss; the rest are counted in `changes_omitted`.
pub const CHANGE_CAP: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Input,
    Env,
    Arg,
    Platform,
    NotInOldLog,
    NotCacheable,
}

impl ChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeKind::Input => "input",
            ChangeKind::Env => "env",
            ChangeKind::Arg => "arg",
            ChangeKind::Platform => "platform",
            ChangeKind::NotInOldLog => "not_in_old_log",
            ChangeKind::NotCacheable => "not_cacheable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delta {
    Modified,
    Added,
    Removed,
    OrderChanged,
}

impl Delta {
    pub fn as_str(self) -> &'static str {
        match self {
            Delta::Modified => "modified",
            Delta::Added => "added",
            Delta::Removed => "removed",
            Delta::OrderChanged => "order_changed",
        }
    }
}

/// One difference. `subject` is an input path, env or platform name, or an
/// argument; env values are never kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub delta: Option<Delta>,
    pub subject: Option<String>,
}

impl Change {
    fn new(kind: ChangeKind, delta: Delta, subject: &str) -> Change {
        Change {
            kind,
            delta: Some(delta),
            subject: Some(subject.to_owned()),
        }
    }

    fn bare(kind: ChangeKind) -> Change {
        Change {
            kind,
            delta: None,
            subject: None,
        }
    }
}

/// An action that ran in the newer log without a cache hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheMiss {
    pub mnemonic: String,
    pub target: String,
    pub primary_output: String,
    pub changes: Vec<Change>,
    pub changes_omitted: usize,
}

/// Shown when a spawn declares no outputs.
pub const NO_OUTPUT: &str = "<no output>";

/// Every cache miss in `new`, in log order, compared with its match in `old`.
pub fn explain(old: &Log, new: &Log) -> Result<Vec<CacheMiss>, ExeclogError> {
    let index = OldIndex::build(old)?;
    let mut misses = Vec::new();
    for (segment, spawn) in new.spawns() {
        if spawn.cache_hit {
            continue;
        }
        let primary = segment.primary_output(spawn)?;
        let mut changes = Vec::new();
        if !spawn.cacheable {
            changes.push(Change::bare(ChangeKind::NotCacheable));
        }
        match index.find(primary.as_deref(), spawn) {
            Some((old_segment, old_spawn)) => {
                compare(old_segment, old_spawn, segment, spawn, &mut changes)?;
            }
            None => changes.push(Change::bare(ChangeKind::NotInOldLog)),
        }
        let omitted = changes.len().saturating_sub(CHANGE_CAP);
        changes.truncate(CHANGE_CAP);
        misses.push(CacheMiss {
            mnemonic: spawn.mnemonic.clone(),
            target: spawn.target_label.clone(),
            primary_output: primary.unwrap_or_else(|| NO_OUTPUT.to_owned()),
            changes,
            changes_omitted: omitted,
        });
    }
    Ok(misses)
}

fn fallback_key(spawn: &Spawn) -> String {
    format!("{}|{}", spawn.target_label, spawn.mnemonic)
}

/// Old-log spawns by primary output, with `target|mnemonic` as the fallback.
struct OldIndex<'a> {
    by_output: HashMap<String, (&'a Segment, &'a Spawn)>,
    by_action: HashMap<String, (&'a Segment, &'a Spawn)>,
}

impl<'a> OldIndex<'a> {
    fn build(log: &'a Log) -> Result<Self, ExeclogError> {
        let mut index = OldIndex {
            by_output: HashMap::new(),
            by_action: HashMap::new(),
        };
        for (segment, spawn) in log.spawns() {
            if let Some(output) = segment.primary_output(spawn)? {
                index.by_output.insert(output, (segment, spawn));
            }
            index
                .by_action
                .insert(fallback_key(spawn), (segment, spawn));
        }
        Ok(index)
    }

    fn find(&self, primary: Option<&str>, spawn: &Spawn) -> Option<(&'a Segment, &'a Spawn)> {
        primary
            .and_then(|output| self.by_output.get(output))
            .or_else(|| self.by_action.get(&fallback_key(spawn)))
            .copied()
    }
}

fn compare(
    old_segment: &Segment,
    old: &Spawn,
    new_segment: &Segment,
    new: &Spawn,
    changes: &mut Vec<Change>,
) -> Result<(), ExeclogError> {
    let old_inputs = old_segment.inputs(old)?;
    let new_inputs = new_segment.inputs(new)?;
    keyed(ChangeKind::Input, &old_inputs, &new_inputs, changes);
    let env = |spawn: &Spawn| -> BTreeMap<String, String> {
        spawn
            .env_vars
            .iter()
            .map(|var| (var.name.clone(), var.value.clone()))
            .collect()
    };
    keyed(ChangeKind::Env, &env(old), &env(new), changes);
    args(&old.args, &new.args, changes);
    let platform = |spawn: &Spawn| -> InputMap {
        spawn
            .platform
            .iter()
            .flat_map(|platform| &platform.properties)
            .map(|property| (property.name.clone(), property.value.clone()))
            .collect()
    };
    keyed(
        ChangeKind::Platform,
        &platform(old),
        &platform(new),
        changes,
    );
    Ok(())
}

/// Compares two maps by key; only keys reach the change list.
fn keyed(
    kind: ChangeKind,
    old: &BTreeMap<String, String>,
    new: &BTreeMap<String, String>,
    changes: &mut Vec<Change>,
) {
    let keys: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    for key in keys {
        match (old.get(key), new.get(key)) {
            (Some(before), Some(after)) if before != after => {
                changes.push(Change::new(kind, Delta::Modified, key));
            }
            (None, Some(_)) => changes.push(Change::new(kind, Delta::Added, key)),
            (Some(_), None) => changes.push(Change::new(kind, Delta::Removed, key)),
            _ => {}
        }
    }
}

/// Arguments as a multiset, plus one `order_changed` when only order differs.
fn args(old: &[String], new: &[String], changes: &mut Vec<Change>) {
    let mut counts: HashMap<&str, i64> = HashMap::new();
    for arg in old {
        *counts.entry(arg).or_default() -= 1;
    }
    for arg in new {
        *counts.entry(arg).or_default() += 1;
    }
    let before = changes.len();
    for arg in old {
        if let Some(count) = counts.get_mut(arg.as_str()) {
            if *count < 0 {
                *count += 1;
                changes.push(Change::new(ChangeKind::Arg, Delta::Removed, arg));
            }
        }
    }
    for arg in new {
        if let Some(count) = counts.get_mut(arg.as_str()) {
            if *count > 0 {
                *count -= 1;
                changes.push(Change::new(ChangeKind::Arg, Delta::Added, arg));
            }
        }
    }
    if changes.len() == before && old != new {
        changes.push(Change {
            kind: ChangeKind::Arg,
            delta: Some(Delta::OrderChanged),
            subject: None,
        });
    }
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
