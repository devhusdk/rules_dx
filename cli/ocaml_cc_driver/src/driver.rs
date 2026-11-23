use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

const PREFIX: &str = "conda-ocaml-";

const LINKING: [&str; 3] = ["cc", "mkdll", "mkexe"];

pub const TOOLS: [&str; 7] = ["ar", "as", "cc", "ld", "mkdll", "mkexe", "ranlib"];

#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub program: String,
    pub args: Vec<OsString>,
}

pub fn tool_of(program: &str) -> Result<&'static str, String> {
    let stem = Path::new(program)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    stem.strip_prefix(PREFIX)
        .and_then(|name| TOOLS.iter().find(|tool| **tool == name))
        .copied()
        .ok_or_else(|| format!("'{program}' is not one of {PREFIX}{{{}}}", TOOLS.join(",")))
}

fn set(env: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    env(name).filter(|value| !value.trim().is_empty())
}

pub fn plan(
    tool: &str,
    env: &dyn Fn(&str) -> Option<String>,
    args: Vec<OsString>,
) -> Result<Plan, String> {
    if tool == "ar"
        && let Some(libtool) = set(env, "DX_OCAML_LIBTOOL")
    {
        let mut rest = args.into_iter().skip(1);
        let out = rest
            .next()
            .ok_or_else(|| format!("{PREFIX}ar: want MODE OUT OBJECTS..."))?;
        let mut planned = vec![OsString::from("-static"), OsString::from("-o"), out];
        planned.extend(rest);
        return Ok(Plan {
            program: libtool,
            args: planned,
        });
    }
    let variable = format!("CONDA_OCAML_{}", tool.to_uppercase());
    let value =
        set(env, &variable).ok_or_else(|| format!("{PREFIX}{tool}: {variable} is not set"))?;
    let mut words = value.split_whitespace().map(OsString::from);
    let program = words.next().unwrap_or_default();
    let mut planned: Vec<OsString> = words.collect();
    if LINKING.contains(&tool)
        && let Some(lib) = set(env, "DX_OCAML_LIB")
    {
        planned.push(OsString::from(format!("-L{lib}")));
    }
    if tool == "as" && set(env, "DX_OCAML_AS_DROP_DEBUG_PREFIX_MAP").is_some() {
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg == "--debug-prefix-map" {
                args.next();
            } else {
                planned.push(arg);
            }
        }
    } else {
        planned.extend(args);
    }
    Ok(Plan {
        program: program.to_string_lossy().into_owned(),
        args: planned,
    })
}

pub fn run(program: &OsStr, args: Vec<OsString>) -> Result<i32, String> {
    let tool = tool_of(&program.to_string_lossy())?;
    let planned = plan(tool, &|name| std::env::var(name).ok(), args)?;
    let status = Command::new(&planned.program)
        .args(&planned.args)
        .status()
        .map_err(|error| format!("{PREFIX}{tool}: cannot run {}: {error}", planned.program))?;
    Ok(status.code().unwrap_or(1))
}
