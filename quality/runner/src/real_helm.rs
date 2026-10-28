use std::cell::{Cell, RefCell};

use super::real_fixtures::*;
use super::*;

const CONFIG_REL: &str = "charts/schemas.kubeconform.json";
const CONFIG: &str = r#"{"schema_version":1,"kubernetes_version":"1.31.0","strict":true,"skip_kinds":[],"core_dir":"schemas.kubeconform/core","crd_dir":"schemas.kubeconform/crd"}"#;
const CONTEXT: &str = r#"{"schema_version":1,"chart_root":"charts/app","values":["charts/app/values.yaml"],"release":"rel","namespace":"ns"}"#;
const DEPLOY: &str = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\nspec:\n  replicas: {{ .Values.replicas }}\n";

thread_local! {
    static LOG: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
    static ENVS: RefCell<Vec<Vec<(String, String)>>> = const { RefCell::new(Vec::new()) };
    static MODE: Cell<u8> = const { Cell::new(0) };
}

fn rendered(source: &str) -> Vec<u8> {
    format!("---\n# Source: renamed/{source}\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\nspec:\n  replicas: \"2\"\n")
        .into_bytes()
}

fn helm_fake(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let args: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    LOG.with(|log| log.borrow_mut().push(args.clone()));
    ENVS.with(|envs| {
        envs.borrow_mut().push(
            env.iter()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
                .collect(),
        );
    });
    let mode = MODE.with(Cell::get);
    let ok = |stdout: Vec<u8>| ChildOutput {
        code: Some(0),
        stdout,
        stderr: Vec::new(),
    };
    match args[1].as_str() {
        "lint" => Ok(ChildOutput {
            code: Some(if mode == 4 { 2 } else { 0 }),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }),
        "template" if mode == 2 => Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: b"Error: template: renamed/templates/deploy.yaml:3:7: executing \"renamed/templates/deploy.yaml\" at <.Values.a.b>: nil pointer evaluating interface {}.b\n".to_vec(),
        }),
        "template" if mode == 5 => Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: b"boom\n".to_vec(),
        }),
        "template" if mode == 3 => Ok(ok(rendered("charts/sub/templates/x.yaml"))),
        "template" => Ok(ok(rendered("templates/deploy.yaml"))),
        _ => {
            let docs: Vec<&String> = args.iter().filter(|a| a.contains("dx-kubeconform")).collect();
            let resources: Vec<_> = docs
                .iter()
                .filter(|_| mode != 0)
                .map(|doc| {
                    serde_json::json!({
                        "filename": doc,
                        "kind": "Deployment",
                        "name": "web",
                        "version": "apps/v1",
                        "status": "statusInvalid",
                        "msg": "jsonschema: does not validate",
                        "validationErrors": [{"path": "/spec/replicas", "msg": "expected integer, but got string"}],
                    })
                })
                .collect();
            Ok(ChildOutput {
                code: Some(i32::from(!resources.is_empty())),
                stdout: serde_json::json!({ "resources": resources }).to_string().into_bytes(),
                stderr: Vec::new(),
            })
        }
    }
}

fn helm_tool(context: Option<&str>, companion: bool) -> RealTool {
    let mut tool_files = vec![
        (CONFIG_REL.to_owned(), CONFIG.as_bytes().to_vec()),
        (
            "charts/schemas.kubeconform/core/pod-v1.json".to_owned(),
            b"{}".to_vec(),
        ),
    ];
    if let Some(context) = context {
        tool_files.push((
            "dx-helm/context.json".to_owned(),
            context.as_bytes().to_vec(),
        ));
    }
    RealTool {
        config_rel: Some(CONFIG_REL.to_owned()),
        tool_files,
        path_tools: if companion {
            vec![(
                "kubeconform".to_owned(),
                PathBuf::from("/fake/bin/kubeconform"),
            )]
        } else {
            Vec::new()
        },
        ..plain_tool()
    }
}

fn chart() -> BTreeMap<String, String> {
    let mut files = single("charts/app/Chart.yaml", "name: renamed\nversion: 0.1.0\n");
    files.insert(
        "charts/app/templates/deploy.yaml".to_owned(),
        DEPLOY.to_owned(),
    );
    files.insert(
        "charts/app/values.yaml".to_owned(),
        "replicas: 2\n".to_owned(),
    );
    files
}

fn run(mode: u8, tool: RealTool) -> Result<Vec<quality_result::proto::Diagnostic>, RunnerError> {
    MODE.with(|m| m.set(mode));
    LOG.with(|log| log.borrow_mut().clear());
    ENVS.with(|envs| envs.borrow_mut().clear());
    backend_for("helm", tool, helm_fake).diagnose("helm", "lint", &chart())
}

fn log() -> Vec<Vec<String>> {
    LOG.with(|log| log.borrow().clone())
}

#[test]
fn helm_runs_lint_template_then_kubeconform_offline() {
    let findings = run(0, helm_tool(Some(CONTEXT), true)).expect("diagnosed");
    assert!(findings.is_empty());
    let calls = log();
    let verbs: Vec<&str> = calls.iter().map(|call| call[1].as_str()).collect();
    assert_eq!(verbs, ["lint", "template", "-output"]);
    assert_eq!(calls[0][0], "/fake/bin/tool");
    assert_eq!(calls[2][0], "/fake/bin/kubeconform");
    assert!(calls[0].contains(&"--kube-version".to_owned()));
    assert!(calls[1].contains(&"rel".to_owned()) && calls[1].contains(&"ns".to_owned()));
    for call in &calls {
        for banned in [
            "dependency",
            "repo",
            "pull",
            "--dependency-update",
            "default",
        ] {
            assert!(
                !call.iter().any(|arg| arg == banned),
                "{banned} in {call:?}"
            );
        }
        assert!(!call.iter().any(|arg| arg.starts_with("http")), "{call:?}");
    }
    ENVS.with(|envs| {
        let envs = envs.borrow();
        for env in &envs[..2] {
            let tmp = &env.iter().find(|(k, _)| k == "TMPDIR").expect("tmpdir").1;
            for key in [
                "HELM_CACHE_HOME",
                "HELM_CONFIG_HOME",
                "HELM_DATA_HOME",
                "HELM_REPOSITORY_CONFIG",
                "HELM_REPOSITORY_CACHE",
                "HELM_REGISTRY_CONFIG",
                "HELM_PLUGINS",
                "KUBECONFIG",
            ] {
                let value = &env
                    .iter()
                    .find(|(k, _)| k == key)
                    .unwrap_or_else(|| panic!("{key}"))
                    .1;
                assert!(value.starts_with(tmp.as_str()), "{key}={value}");
            }
            assert!(env.contains(&("HELM_NO_PLUGINS".to_owned(), "1".to_owned())));
        }
        assert!(envs[2].iter().all(|(k, _)| !k.starts_with("HELM_")));
    });
}

#[test]
fn helm_maps_invalid_rendered_documents_to_their_template() {
    let findings = run(1, helm_tool(Some(CONTEXT), true)).expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "charts/app/templates/deploy.yaml");
    assert_eq!(findings[0].tool_id, "helm");
    assert_eq!(findings[0].start_byte, Some(0));
}

#[test]
fn helm_maps_subchart_documents_to_chart_yaml() {
    let findings = run(3, helm_tool(Some(CONTEXT), true)).expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "charts/app/Chart.yaml");
    assert!(
        findings[0].message.contains("charts/sub/templates/x.yaml"),
        "{}",
        findings[0].message
    );
}

#[test]
fn helm_template_failure_skips_kubeconform() {
    let findings = run(2, helm_tool(Some(CONTEXT), true)).expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "charts/app/templates/deploy.yaml");
    assert_eq!(log().len(), 2);
}

#[test]
fn helm_fails_closed() {
    run(0, helm_tool(None, true)).expect_err("missing context");
    run(0, helm_tool(Some(CONTEXT), false)).expect_err("missing companion");
    let mut no_config = helm_tool(Some(CONTEXT), true);
    no_config.config_rel = None;
    let err = run(0, no_config).expect_err("missing config");
    assert!(err.to_string().contains("kubernetes_schemas"), "{err}");
    run(4, helm_tool(Some(CONTEXT), true)).expect_err("unknown lint exit");
    run(5, helm_tool(Some(CONTEXT), true)).expect_err("template failure without Error line");
    let escaping = CONTEXT.replace("charts/app\"", "../app\"");
    run(0, helm_tool(Some(&escaping), true)).expect_err("chart root escapes");
    let mut delegated = helm_tool(Some(CONTEXT), true);
    delegated.upstream_diagnostics = vec![PathBuf::from("/nope")];
    run(0, delegated).expect_err("no delegated mode");
}

#[test]
fn helm_is_check_only() {
    MODE.with(|m| m.set(0));
    let backend = backend_for("helm", helm_tool(Some(CONTEXT), true), helm_fake);
    assert_eq!(
        backend
            .apply_fix("helm", "charts/app/templates/deploy.yaml", DEPLOY, "lint")
            .expect("check-only"),
        DEPLOY
    );
}
