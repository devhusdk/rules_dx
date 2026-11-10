//! `dx_progenitor`: writes one Rust client file from an OpenAPI document.
//!
//! Usage: `dx_progenitor SPEC OUT`. SPEC is YAML or JSON. The output parses as
//! Rust and uses the `progenitor-client` runtime, which the consumer provides.

use std::process::ExitCode;

use progenitor::{GenerationSettings, Generator, InterfaceStyle};

fn generate(spec: &str) -> Result<String, String> {
    let api: openapiv3::OpenAPI =
        serde_yaml::from_str(spec).map_err(|e| format!("parse spec: {e}"))?;
    let mut generator =
        Generator::new(GenerationSettings::default().with_interface(InterfaceStyle::Positional));
    let tokens = generator
        .generate_tokens(&api)
        .map_err(|e| format!("generate: {e}"))?;
    let file = syn::parse2(tokens).map_err(|e| format!("generated code does not parse: {e}"))?;
    Ok(prettyplease::unparse(&file))
}

fn run(args: &[String]) -> Result<(), String> {
    let [spec, out] = args else {
        return Err("usage: dx_progenitor SPEC OUT".to_owned());
    };
    let text = std::fs::read_to_string(spec).map_err(|e| format!("read {spec}: {e}"))?;
    let code = generate(&text)?;
    std::fs::write(out, code).map_err(|e| format!("write {out}: {e}"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("dx_progenitor: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_openapi_input() {
        assert!(generate("not: an api").is_err());
    }

    #[test]
    fn needs_two_arguments() {
        assert!(run(&[]).is_err());
    }
}
