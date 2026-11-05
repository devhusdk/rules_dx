# npm hub from MODULE.bazel

Verifies the load of `npm_link_all_packages` names the hub that
`npm_translate_lock` declares in `MODULE.bazel`, at the lock directory and in
each workspace package, replacing an existing `@npm` load.
