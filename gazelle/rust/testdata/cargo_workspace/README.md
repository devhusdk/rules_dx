# Cargo Workspace Dependencies

Verifies `workspace = true` dependencies, inherited `[workspace.package]` version and edition, renamed workspace entries, local workspace path deps (including a renamed one, `apiv = { path = ..., package = "api" }`), and `[target.'cfg(..)'.dependencies]` sections.
