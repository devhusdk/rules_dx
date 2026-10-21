# Quickstart Guide

Fixture-scale quickstart for the docs site pipeline. Every step below is
executed in CI, so this guide cannot rot: a step that stops working fails
the pipeline instead of silently going stale.

## Steps

1. Extract the demo IR shard:

   ```sh
   bazel build //tools/docs/site:demo_extract
   ```

2. Aggregate shards plus prose into render inputs:

   ```sh
   bazel build //tools/docs/site:demo_aggregate
   ```

3. Run the site unit checks:

   ```sh
   bazel test //tools/docs/site:site_unit
   ```
