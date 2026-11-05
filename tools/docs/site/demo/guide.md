# Quickstart Guide

Fixture-scale quickstart for the docs site pipeline. Every step below is
executed in CI, so this guide cannot rot: a step that stops working fails
the pipeline instead of silently going stale.

## Steps

1. Render the fixture crate's API and the prose into a site:

   ```sh
   bazel build //tools/docs/site:demo_site
   ```

2. Run the site unit checks:

   ```sh
   bazel test //tools/docs/site:site_unit
   ```
