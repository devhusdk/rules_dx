//! Generates the Rust crate sources of one `capnp_rust_library`.
//!
//! The driver runs the pinned `capnp` compiler over the staged import closure,
//! reads the `CodeGeneratorRequest` it writes to stdout, maps every import to the
//! crate that owns its generated code, and runs the `capnpc` code generator.
//! It writes one `<stem>_capnp.rs` per owned schema and a `lib.rs` that declares
//! them as top-level modules. Any failure exits non-zero and never leaves a
//! partial crate: the caller declares every output, so a missing one fails the action.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use capnp::schema_capnp::{code_generator_request, node};

const STD_PREFIX: &str = "capnp/";
const STD_SCHEMA: &str = "capnp/schema.capnp";

#[derive(Debug, Default)]
struct Args {
    capnp: PathBuf,
    root: PathBuf,
    out: PathBuf,
    own: Vec<String>,
    provide: BTreeMap<String, String>,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = raw.iter();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "--capnp" => args.capnp = PathBuf::from(value("--capnp")?),
            "--root" => args.root = PathBuf::from(value("--root")?),
            "--out" => args.out = PathBuf::from(value("--out")?),
            "--own" => args.own.push(value("--own")?),
            "--provide" => {
                let spec = value("--provide")?;
                let (path, krate) = spec
                    .split_once('=')
                    .filter(|(p, c)| !p.is_empty() && !c.is_empty())
                    .ok_or_else(|| format!("--provide wants IMPORT_PATH=CRATE, got {spec}"))?;
                if args.provide.insert(path.into(), krate.into()).is_some() {
                    return Err(format!("--provide given twice for {path}"));
                }
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if args.capnp.as_os_str().is_empty() || args.root.as_os_str().is_empty() {
        return Err("--capnp and --root are required".into());
    }
    if args.out.as_os_str().is_empty() {
        return Err("--out is required".into());
    }
    if args.own.is_empty() {
        return Err("at least one --own schema is required".into());
    }
    Ok(args)
}

/// The module name capnpc gives a schema: the file stem with `-` as `_`, plus `_capnp`.
fn module_name(path: &str) -> Result<String, String> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file
        .strip_suffix(".capnp")
        .ok_or_else(|| format!("{path} is not a .capnp schema"))?;
    if stem.is_empty() {
        return Err(format!("{path} has an empty file stem"));
    }
    Ok(format!("{}_capnp", stem.replace('-', "_")))
}

/// The generated file for an owned schema, relative to the output directory.
fn generated_path(path: &str) -> Result<String, String> {
    let module = module_name(path)?;
    let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
    Ok(if dir.is_empty() {
        format!("{module}.rs")
    } else {
        format!("{dir}/{module}.rs")
    })
}

/// Checks the owned schemas and returns their module names in order.
fn owned_modules(own: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for path in own {
        let module = module_name(path)?;
        if let Some(first) = seen.insert(module.clone(), path.clone()) {
            return Err(format!(
                "schemas {first} and {path} in one capnp_rust_library both generate module {module}; \
                 rename one or split them into separate libraries"
            ));
        }
    }
    Ok(seen.into_iter().map(|(m, p)| (p, m)).collect())
}

/// Reads the request and maps the id of every provided import to its crate.
///
/// `own` is the set of schemas this crate owns. Every import of a requested file
/// must be owned, provided by a dependency crate, or a bundled `/capnp/` schema.
fn resolve_provides(
    request: code_generator_request::Reader,
    own: &BTreeSet<String>,
    provide: &BTreeMap<String, String>,
) -> Result<HashMap<u64, String>, String> {
    let nodes = request.get_nodes().map_err(|e| e.to_string())?;
    let mut names: HashMap<u64, String> = HashMap::new();
    for n in nodes.iter() {
        if matches!(n.which(), Ok(node::File(()))) {
            let name = n
                .get_display_name()
                .map_err(|e| e.to_string())?
                .to_string()
                .map_err(|e| e.to_string())?;
            names.insert(n.get_id(), name);
        }
    }
    let requested = request.get_requested_files().map_err(|e| e.to_string())?;
    let mut requested_names = BTreeSet::new();
    let mut map: HashMap<u64, String> = HashMap::new();
    for file in requested.iter() {
        let filename = file
            .get_filename()
            .map_err(|e| e.to_string())?
            .to_string()
            .map_err(|e| e.to_string())?;
        if !own.contains(&filename) {
            return Err(format!(
                "capnp compiled {filename}, which is not an owned schema"
            ));
        }
        requested_names.insert(filename.clone());
        for import in file.get_imports().map_err(|e| e.to_string())?.iter() {
            let id = import.get_id();
            let written = import
                .get_name()
                .map_err(|e| e.to_string())?
                .to_string()
                .map_err(|e| e.to_string())?;
            let path = names.get(&id).ok_or_else(|| {
                format!("{filename}: import {written} has no file node in the request")
            })?;
            if own.contains(path) {
                continue;
            }
            if let Some(krate) = provide.get(path) {
                map.insert(id, krate.clone());
            } else if path == STD_SCHEMA {
                map.insert(id, "capnp".to_string());
            } else if path.starts_with(STD_PREFIX) {
                // Bundled annotation schemas such as c++.capnp and java.capnp generate
                // no code; a type from json.capnp or stream.capnp fails to compile in rustc.
            } else {
                return Err(format!(
                    "{filename}: import {written} resolves to /{path}, which is neither owned by this \
                     capnp_rust_library nor provided by a dependency; add the capnp_rust_library that \
                     generates it to deps"
                ));
            }
        }
    }
    for want in own {
        if !requested_names.contains(want) {
            return Err(format!("capnp did not compile the owned schema {want}"));
        }
    }
    Ok(map)
}

fn lib_rs(modules: &[(String, String)]) -> Result<String, String> {
    let mut text = String::from("// @generated by capnpc_rust_dx. DO NOT EDIT.\n\n");
    for (path, module) in modules {
        text.push_str(&format!(
            "#[path = \"{}\"]\npub mod {module};\n",
            generated_path(path)?
        ));
    }
    Ok(text)
}

fn run(args: &Args) -> Result<(), String> {
    let modules = owned_modules(&args.own)?;
    let own: BTreeSet<String> = args.own.iter().cloned().collect();

    let mut command = Command::new(&args.capnp);
    // capnp warns when PWD does not match the working directory.
    command
        .env_remove("PWD")
        .args(["compile", "-o", "-", "--no-standard-import"])
        .arg(format!("--import-path={}", args.root.display()))
        .arg(format!("--src-prefix={}", args.root.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for path in &args.own {
        command.arg(args.root.join(path));
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run {}: {e}", args.capnp.display()))?;
    if !output.status.success() {
        return Err(format!("capnp compile failed with {}", output.status));
    }

    let message =
        capnp::serialize::read_message(&output.stdout[..], capnp::message::ReaderOptions::new())
            .map_err(|e| format!("capnp wrote an unreadable CodeGeneratorRequest: {e}"))?;
    let request = message
        .get_root::<code_generator_request::Reader>()
        .map_err(|e| format!("capnp wrote an unreadable CodeGeneratorRequest: {e}"))?;
    let provides = resolve_provides(request, &own, &args.provide)?;

    let mut generator = capnpc::codegen::CodeGenerationCommand::new();
    generator
        .output_directory(&args.out)
        .crates_provide_map(provides);
    generator
        .run(&output.stdout[..])
        .map_err(|e| format!("capnpc code generation failed: {e}"))?;

    for path in &args.own {
        let generated = args.out.join(generated_path(path)?);
        if !Path::new(&generated).is_file() {
            return Err(format!("capnpc did not write {}", generated.display()));
        }
    }
    std::fs::write(args.out.join("lib.rs"), lib_rs(&modules)?)
        .map_err(|e| format!("cannot write lib.rs: {e}"))
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let result = parse_args(&raw).and_then(|args| run(&args));
    if let Err(message) = result {
        eprintln!("capnpc_rust_dx: {message}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        files: &[(&str, &[(&str, u64)])],
        extra: &[(&str, u64)],
    ) -> capnp::message::Builder<capnp::message::HeapAllocator> {
        let mut message = capnp::message::Builder::new_default();
        {
            let mut root = message.init_root::<code_generator_request::Builder>();
            let mut all: Vec<(String, u64)> =
                extra.iter().map(|(n, i)| (n.to_string(), *i)).collect();
            for (index, (name, _)) in files.iter().enumerate() {
                all.push((name.to_string(), 1000 + index as u64));
            }
            let mut nodes = root.reborrow().init_nodes(all.len() as u32);
            for (index, (name, id)) in all.iter().enumerate() {
                let mut n = nodes.reborrow().get(index as u32);
                n.set_id(*id);
                n.set_display_name(name);
                n.set_file(());
            }
            let mut requested = root.init_requested_files(files.len() as u32);
            for (index, (name, imports)) in files.iter().enumerate() {
                let mut f = requested.reborrow().get(index as u32);
                f.set_id(1000 + index as u64);
                f.set_filename(name);
                let mut list = f.init_imports(imports.len() as u32);
                for (j, (iname, iid)) in imports.iter().enumerate() {
                    let mut i = list.reborrow().get(j as u32);
                    i.set_id(*iid);
                    i.set_name(iname);
                }
            }
        }
        message
    }

    fn resolve(
        files: &[(&str, &[(&str, u64)])],
        extra: &[(&str, u64)],
        provide: &[(&str, &str)],
    ) -> Result<HashMap<u64, String>, String> {
        let message = request(files, extra);
        let reader = message
            .get_root_as_reader::<code_generator_request::Reader>()
            .unwrap();
        let own = files.iter().map(|(n, _)| n.to_string()).collect();
        let provide = provide
            .iter()
            .map(|(p, c)| (p.to_string(), c.to_string()))
            .collect();
        resolve_provides(reader, &own, &provide)
    }

    #[test]
    fn maps_provided_imports_and_schema_capnp() {
        let map = resolve(
            &[(
                "a/shape.capnp",
                &[
                    ("/b/common.capnp", 5),
                    ("/capnp/schema.capnp", 6),
                    ("/capnp/c++.capnp", 7),
                ],
            )],
            &[
                ("b/common.capnp", 5),
                ("capnp/schema.capnp", 6),
                ("capnp/c++.capnp", 7),
            ],
            &[("b/common.capnp", "base_rs")],
        )
        .unwrap();
        assert_eq!(map.get(&5).map(String::as_str), Some("base_rs"));
        assert_eq!(map.get(&6).map(String::as_str), Some("capnp"));
        assert!(!map.contains_key(&7));
    }

    #[test]
    fn owned_imports_need_no_crate() {
        let map = resolve(
            &[
                ("a/kind.capnp", &[]),
                ("a/shape.capnp", &[("kind.capnp", 1000)]),
            ],
            &[],
            &[],
        )
        .unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn unresolved_import_names_the_import_and_the_fix() {
        let err = resolve(
            &[("a/shape.capnp", &[("/b/common.capnp", 5)])],
            &[("b/common.capnp", 5)],
            &[],
        )
        .unwrap_err();
        assert!(
            err.contains("/b/common.capnp") && err.contains("capnp_rust_library"),
            "{err}"
        );
    }

    #[test]
    fn rejects_a_compiled_file_that_is_not_owned() {
        let message = request(&[("a/other.capnp", &[])], &[]);
        let reader = message
            .get_root_as_reader::<code_generator_request::Reader>()
            .unwrap();
        let own = BTreeSet::from(["a/mine.capnp".to_string()]);
        assert!(resolve_provides(reader, &own, &BTreeMap::new()).is_err());
    }

    #[test]
    fn duplicate_module_stems_fail() {
        let own = vec!["a/common.capnp".to_string(), "b/common.capnp".to_string()];
        let err = owned_modules(&own).unwrap_err();
        assert!(err.contains("common_capnp"), "{err}");
    }

    #[test]
    fn hyphens_become_underscores() {
        assert_eq!(
            generated_path("p/my-shape.capnp").unwrap(),
            "p/my_shape_capnp.rs"
        );
        assert!(module_name("x.txt").is_err());
    }

    #[test]
    fn parses_arguments_and_rejects_bad_ones() {
        let ok: Vec<String> = [
            "--capnp",
            "c",
            "--root",
            "r",
            "--out",
            "o",
            "--own",
            "a.capnp",
            "--provide",
            "b.capnp=krate",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let args = parse_args(&ok).unwrap();
        assert_eq!(
            args.provide.get("b.capnp").map(String::as_str),
            Some("krate")
        );
        assert!(parse_args(&["--bogus".to_string()]).is_err());
        let mut bad = ok.clone();
        bad.push("--provide".into());
        bad.push("nocrate".into());
        assert!(parse_args(&bad).is_err());
    }

    #[test]
    fn lib_rs_lists_modules_with_paths() {
        let modules = owned_modules(&["p/a.capnp".to_string()]).unwrap();
        let text = lib_rs(&modules).unwrap();
        assert!(
            text.contains("#[path = \"p/a_capnp.rs\"]\npub mod a_capnp;"),
            "{text}"
        );
    }
}
