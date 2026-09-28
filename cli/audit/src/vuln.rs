#[path = "vuln_go.rs"]
mod vuln_go;
#[path = "vuln_match.rs"]
mod vuln_match;
#[path = "vuln_maven.rs"]
mod vuln_maven;
#[path = "vuln_nuget.rs"]
mod vuln_nuget;
#[path = "vuln_snapshot.rs"]
mod vuln_snapshot;
#[path = "vuln_types.rs"]
mod vuln_types;

pub use vuln_go::*;
pub use vuln_match::*;
pub use vuln_maven::*;
pub use vuln_nuget::*;
pub use vuln_snapshot::*;
pub use vuln_types::*;

#[cfg(test)]
pub(crate) use vuln_go::strip_go_v;
#[cfg(test)]
pub(crate) use vuln_snapshot::{
    interval_to_scope, project_osv_snapshot, range_events_to_intervals,
};

#[cfg(test)]
#[path = "vuln_tests.rs"]
mod vuln_tests;
