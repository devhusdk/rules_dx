//! Host-run qualifier: checks exported compile commands against Bazel's own action argv.
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use cc_context::{AqueryGraph, Error};

const USAGE: &str = "Usage: bazel run //cc/private/context:qualify -- --target <label>... \
[--compiler <name>] [--output <file>] [--bazel <bin>] [-- <bazel flags>...]";

const IDENTITIES: [&str; 2] = ["//cc/private/context:identity", "//cc/private/context:identity_exec"];

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

struct Runner {
    bazel: String,
    cwd: PathBuf,
    log: Vec<String>,
}

impl Runner {
    fn exec(&mut self, program: &str, args: &[String]) -> Result<(String, String), Error> {
        self.log.push(format!("{program} {}", args.join(" ")));
        let out = Command::new(program)
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .map_err(|err| Error::Usage(format!("cannot run '{program}': {err}")))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() {
            return Err(Error::Usage(format!(
                "'{program} {}' failed: {stderr}",
                args.join(" ")
            )));
        }
        Ok((stdout, stderr))
    }

    fn bazel(&mut self, verb: &[&str], extra: &[String]) -> Result<String, Error> {
        let bazel = self.bazel.clone();
        let args: Vec<String> = verb
            .iter()
            .map(|s| s.to_string())
            .chain(extra.iter().cloned())
            .collect();
        Ok(self.exec(&bazel, &args)?.0)
    }
}

fn run(args: Vec<String>) -> Result<(), Error> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let mut targets = Vec::new();
    // The version probe runs the compiler itself, so its files must be on local disk: a
    // remote cache hit would leave a remotely cached repository unmaterialized.
    let mut flags =
        vec!["--modify_execution_info=CppCompile=+no-remote,CppCompile=+no-cache".to_string()];
    let mut output = None;
    let mut bazel = "bazel".to_string();
    let mut compiler = None;
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        if flag == "--" {
            flags.extend(iter.by_ref());
            break;
        }
        let value = iter
            .next()
            .ok_or_else(|| Error::Usage(format!("'{flag}' needs a value")))?;
        match flag.as_str() {
            "--target" => targets.push(value),
            "--compiler" => {
                flags.push(format!("--//cc:compiler={value}"));
                compiler = Some(value);
            }
            "--output" => output = Some(value),
            "--bazel" => bazel = value,
            _ => return Err(Error::Usage(format!("unknown flag '{flag}'"))),
        }
    }
    if targets.is_empty() {
        return Err(Error::Usage("'--target' is required".to_string()));
    }
    let cwd = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .map_err(|_| Error::Usage("run through 'bazel run' (no workspace)".to_string()))?;
    let mut runner = Runner {
        bazel,
        cwd: cwd.clone(),
        log: Vec::new(),
    };
    let mut build: Vec<String> = flags.clone();
    build.extend(targets.iter().cloned());
    build.extend(IDENTITIES.iter().map(|s| s.to_string()));
    runner.bazel(&["build"], &build)?;

    let mut cquery = vec!["--output=files".to_string()];
    cquery.extend(flags.iter().cloned());
    cquery.push(IDENTITIES.join(" + "));
    let files = runner.bazel(&["cquery"], &cquery)?;
    let identity_paths: Vec<PathBuf> = files
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| cwd.join(line.trim()))
        .collect();

    let universe = targets.join(" + ");
    let mut aquery = vec![
        "--include_param_files".to_string(),
        "--output=jsonproto".to_string(),
    ];
    aquery.extend(flags.iter().cloned());
    aquery.push(format!("mnemonic(\"CppCompile\", deps({universe}))"));
    let record = runner.bazel(&["aquery"], &aquery)?;
    let execroot = runner
        .bazel(&["info"], &["execution_root".to_string()])?
        .trim()
        .to_string();
    let output_base = runner
        .bazel(&["info"], &["output_base".to_string()])?
        .trim()
        .to_string();

    let mut graph = AqueryGraph::parse(&record)?;
    graph.retain_owned(&targets);
    let identities = cc_context::read_identities(&identity_paths)?;
    let commands = graph.compile_commands(&PathBuf::from(&execroot), &identities)?;
    if commands.is_empty() {
        return Err(Error::Usage(
            "aquery returned no compile action".to_string(),
        ));
    }
    let qualified = cc_context::qualify(&graph, &commands, &PathBuf::from(&execroot))?;

    let (version, _) = runner.exec("git", &["rev-parse".to_string(), "HEAD".to_string()])?;
    let bazel_version = runner.bazel(&["version"], &[])?;
    let banner = match commands.first().and_then(|c| c.arguments.first()) {
        Some(argv0) => {
            let (program, args) = cc_context::version_probe(
                &PathBuf::from(&execroot),
                &PathBuf::from(&output_base),
                argv0,
            );
            let (out, err) = runner.exec(&program.to_string_lossy(), &args)?;
            format!("{out}{err}")
        }
        None => String::new(),
    };
    let evidence = serde_json::json!({
        "sha": version.trim(),
        "bazel_version": bazel_version,
        "compiler_flag": compiler,
        "compiler_banner": banner,
        "commands": runner.log,
        "qualified": qualified,
    });
    let text =
        serde_json::to_string_pretty(&evidence).map_err(|err| Error::UnserializableDatabase {
            reason: err.to_string(),
        })?;
    match output {
        Some(path) => {
            std::fs::write(&path, text + "\n").map_err(|err| Error::UnwritableOutput {
                path,
                reason: err.to_string(),
            })?
        }
        None => println!("{text}"),
    }
    println!("qualify: {} compile command(s) qualified", qualified.len());
    Ok(())
}
