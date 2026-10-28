use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const ROOT: &str = r#"openapi: 3.1.0
info:
  title: Pets
  version: 1.0.0
paths:
  /pets/{petId}:
    get:
      operationId: getPet
      parameters:
        - name: petId
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: One pet.
          content:
            application/json:
              schema:
                $ref: schemas/pet.yaml#/Pet
"#;

const PET: &str = r#"Pet:
  type: object
  properties:
    id:
      type: string
    owner:
      $ref: ../shared/owner.yaml#/Owner
"#;

const BAD_PET: &str = r#"Pet:
  type: object
  properties:
    id:
      type: 7
    owner:
      $ref: ../shared/owner.yaml#/Owner
"#;

const OWNER: &str = "Owner:\n  type: object\n";

const DUPLICATE: &str = r#"openapi: 3.1.0
info:
  title: Pets
  version: 1.0.0
paths:
  /pets:
    get:
      operationId: listPets
      responses:
        "200":
          description: Pets.
  /owners:
    get:
      operationId: listPets
      responses:
        "200":
          description: Owners.
"#;

const SWAGGER: &str = "swagger: \"2.0\"\ninfo:\n  title: Pets\n  version: 1.0.0\npaths: {}\n";

const REMOTE: &str = "openapi: 3.1.0\ninfo:\n  title: Pets\n  version: 1.0.0\npaths: {}\ncomponents:\n  schemas:\n    Pet:\n      $ref: https://example.com/pet.yaml\n";

const PLUGINS_CONFIG: &str = "extends:\n  - spec\nplugins:\n  - ./plugin.js\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "api_spec_family",
    family_id = "api_spec",
    lint = ["redocly"],
)

workspace_policy(
    name = "policy",
    families = [":api_spec_family"],
)
"#;

const API_BUILD: &str = r#"load("@rules_dx//api_spec/rules:defs.bzl", "api_spec_library")

api_spec_library(
    name = "api",
    srcs = ["openapi.yaml"],
    refs = ["schemas/pet.yaml"],
    deps = ["//shared"],
)
"#;

const SHARED_BUILD: &str = r#"load("@rules_dx//api_spec/rules:defs.bzl", "api_spec_library")

api_spec_library(
    name = "shared",
    refs = ["owner.yaml"],
    visibility = ["//visibility:public"],
)
"#;

const CONFIGURED_BUILD: &str = r#"load("@rules_dx//api_spec/rules:defs.bzl", "api_spec_library")
load("@rules_dx//quality:native_config.bzl", "redocly_config")

redocly_config(
    name = "redocly",
    src = "redocly.yaml",
)

api_spec_library(
    name = "api",
    srcs = ["openapi.yaml"],
    refs = ["schemas/pet.yaml"],
    aspect_hints = [":redocly"],
    deps = ["//shared"],
)
"#;

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("api/BUILD.bazel"), API_BUILD);
    write(&root.join("api/openapi.yaml"), ROOT);
    write(&root.join("api/schemas/pet.yaml"), PET);
    write(&root.join("shared/BUILD.bazel"), SHARED_BUILD);
    write(&root.join("shared/owner.yaml"), OWNER);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("api-spec-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn a_clean_root_with_refs_and_deps_passes() {
    let root = consumer();
    let clean = dx(&root, "lint", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
}

#[test]
fn findings_in_the_root_keep_their_position() {
    let root = consumer();
    write(&root.join("api/openapi.yaml"), DUPLICATE);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("operation-operationId-unique"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("api/openapi.yaml:14:"),
        "{}",
        out.combined()
    );
}

#[test]
fn findings_in_a_ref_land_on_the_root() {
    let root = consumer();
    write(&root.join("api/schemas/pet.yaml"), BAD_PET);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("api/openapi.yaml:1:1"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("api/schemas/pet.yaml:5:"),
        "{}",
        out.combined()
    );
}

#[test]
fn swagger_and_remote_refs_are_rejected_offline() {
    let root = consumer();
    for (text, rule) in [
        (SWAGGER, "dx/api-spec-root"),
        (REMOTE, "dx/api-spec-remote-ref"),
    ] {
        write(&root.join("api/openapi.yaml"), text);
        let out = dx(&root, "lint", &[]);
        assert_eq!(out.status.code(), Some(1), "{rule}: {}", out.combined());
        assert!(out.combined().contains(rule), "{rule}: {}", out.combined());
    }
}

#[test]
fn a_config_that_loads_plugins_is_never_clean() {
    let root = consumer();
    write(&root.join("api/BUILD.bazel"), CONFIGURED_BUILD);
    write(&root.join("api/redocly.yaml"), PLUGINS_CONFIG);
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("`plugins` is not allowed"),
        "{}",
        out.combined()
    );
}

#[test]
fn lint_apply_never_rewrites_a_spec() {
    let root = consumer();
    let spec = root.join("api/openapi.yaml");
    write(&spec, DUPLICATE);
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&spec), DUPLICATE);
}
