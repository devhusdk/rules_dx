# Quickstart Guide

Fixture-scale quickstart for the docs site pipeline. Every step below is
executed in CI, so this guide cannot rot: a step that stops working fails
the pipeline instead of silently going stale.

## Steps

1. Build the fixture crate's API shard:

   ```sh
   bazel build //tools/docs/rust/fixture:docs_fixture_docs
   ```

2. Render the shard and the prose into a site:

   ```sh
   bazel build //tools/docs/site:demo_site
   ```

3. Run the site unit checks:

   ```sh
   bazel test //tools/docs/site:site_unit
   ```
