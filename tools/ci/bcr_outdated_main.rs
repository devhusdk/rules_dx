//! `bcr_outdated_bin <MODULE.bazel>` prints `bazel:<module> <version>` for each outdated dep.

use std::process::ExitCode;

fn fetch(agent: &ureq::Agent, module: &str) -> Result<String, String> {
    let url = format!("https://bcr.bazel.build/modules/{module}/metadata.json");
    let mut response = agent.get(&url).call().map_err(|e| e.to_string())?;
    if response.status().as_u16() != 200 {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path] = args.as_slice() else {
        eprintln!("usage: bcr_outdated_bin <MODULE.bazel>");
        return ExitCode::from(2);
    };
    let parsed = std::fs::read_to_string(path)
        .map_err(|e| format!("{path}: {e}"))
        .and_then(|text| dx_module_bazel::ModuleFile::parse(&text).map_err(|e| e.to_string()));
    let module = match parsed {
        Ok(module) => module,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let agent = dx_deploy_tools::http::publish_agent();
    let (lines, errors) = bcr_outdated::outdated(&module, &|name| fetch(&agent, name));
    for line in &lines {
        println!("{line}");
    }
    for error in &errors {
        eprintln!("{error}");
    }
    ExitCode::from(u8::from(!errors.is_empty()))
}
