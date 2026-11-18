# Examples

These examples help you start a repository or a CI setup with rules_dx. Copy the one you need into your own repository.

- [consumer-ci](https://github.com/devhusdk/rules_dx/tree/main/examples/consumer-ci): a GitHub Actions caller for the consumer workflow.
- [docs-ci](docs-ci/): a GitHub Actions caller for the docs workflow.
- [gitlab-ci](gitlab-ci/): the GitLab CI setup that `dx init --ci gitlab` writes, with its inputs and remote cache.
- [adopt-polyglot](adopt-polyglot/): a Python, Rust, JavaScript, TypeScript and C++ tree set up with `dx generate`.
- [shared-rust](https://github.com/devhusdk/rules_dx/tree/main/examples/shared-rust): one Rust task board built for the command line, the browser, WASI, Android and Apple.

Build and test every example from a rules_dx checkout:

```sh
tools/dx test //examples/...
```
