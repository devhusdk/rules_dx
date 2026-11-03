# IaC example

One Terraform module, one CloudFormation template and one Bicep module with a dependency.

| Package | Role |
| --- | --- |
| `terraform` | `greeting` module with one variable and one output |
| `cloudformation` | `bucket` template with one S3 bucket |
| `bicep` | `main` module that calls `lib` |

```sh
bazel build //examples/iac/...
bazel run @rules_dx//:dx -- check //examples/iac/...
bazel run @rules_dx//:dx -- format //examples/iac/...
```
