# Helm test data

`dep-0.1.0.tgz` is a vendored subchart for the Helm consumer test. It holds
`dep/Chart.yaml` (apiVersion v2, name dep, version 0.1.0) and
`dep/templates/configmap.yaml` (one ConfigMap named dep). It was built with:

```
tar --sort=name --mtime='UTC 2020-01-01' --owner=0 --group=0 --numeric-owner -cf - dep | gzip -n -9 > dep-0.1.0.tgz
```
