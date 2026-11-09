# Entry thin-binary generation

Verifies a recognized `main.ts` entry generates one `typescript_binary`
and no library unless another source imports it. Only the exact `main` basename is an entry; other
layouts are an explicit wont-fix per issue #585.

The fixture root has a `package.json`, so the generated test gets
`package_json = "//:package_json"`. The hand-written `widget_test` sets
a nearer manifest, and regeneration must keep it.
