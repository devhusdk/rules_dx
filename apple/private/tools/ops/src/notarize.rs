//! Notarizes and staples a signed macOS app.

use std::path::PathBuf;

use crate::signing::{
    Flags, absolute, caller, describe, hardened, outside_bazel_out, private_files, text, value,
};
use crate::util::{Env, Failure, Outcome, Scratch, exit_code, reject, usage};
use crate::xcode::{DITTO, SPCTL, Tools};

pub const PREFIX: &str = "dx-apple-notarize";

const HELP: &str = "usage: bazel run <notarize> -- --app <App.app> (--keychain-profile <name> | --key <AuthKey.p8> --key-id <id> --issuer <uuid>) [--timeout <duration>]";

fn combined(out: &crate::util::Output) -> String {
    format!("{}{}", out.stdout, out.stderr)
}

/// Notarizes the app `--app` names; `args[0]` is the declared bundle id.
pub fn run(tools: &Tools, args: &[String], env: Env) -> Outcome {
    private_files();
    let [bundle_id, flag_args @ ..] = args else {
        return Err(usage("run the notarize launcher with bazel run"));
    };
    let flags = Flags::parse(
        flag_args,
        &[
            "app",
            "keychain-profile",
            "key",
            "key-id",
            "issuer",
            "timeout",
        ],
        HELP,
    )?;
    let (profile, key, key_id, issuer) = (
        flags.get("keychain-profile"),
        flags.get("key"),
        flags.get("key-id"),
        flags.get("issuer"),
    );
    let timeout = if flag_args.iter().any(|arg| arg.starts_with("--timeout")) {
        flags.get("timeout")
    } else {
        "2h"
    };
    if flags.get("app").is_empty() {
        return Err(usage(format!(
            "--app names the signed .app to notarize; {HELP}"
        )));
    }
    if timeout.is_empty() {
        return Err(usage(format!("--timeout needs a duration like 2h; {HELP}")));
    }
    let given = [key, key_id, issuer];
    if !profile.is_empty() {
        if given.iter().any(|value| !value.is_empty()) {
            return Err(usage(format!(
                "pass --keychain-profile or --key with --key-id and --issuer, not both; {HELP}"
            )));
        }
    } else if given.iter().any(|value| !value.is_empty()) {
        if given.iter().any(|value| value.is_empty()) {
            return Err(usage(format!(
                "--key, --key-id and --issuer go together; {HELP}"
            )));
        }
    } else {
        return Err(usage(format!(
            "pass --keychain-profile or --key with --key-id and --issuer; {HELP}"
        )));
    }

    let caller = caller(env);
    let app = absolute(flags.get("app"), &caller);
    if !text(&app).ends_with(".app") {
        return Err(usage(format!(
            "--app {} must name an .app bundle",
            app.display()
        )));
    }
    if !app.is_dir() {
        return Err(usage(format!(
            "--app {} is not a bundle directory",
            app.display()
        )));
    }
    let parent = app
        .parent()
        .unwrap_or(&caller)
        .canonicalize()
        .map_err(|e| usage(e.to_string()))?;
    let app = parent.join(app.file_name().unwrap_or_default());
    let key = if key.is_empty() {
        PathBuf::new()
    } else {
        absolute(key, &caller)
    };
    if !key.as_os_str().is_empty() && !key.is_file() {
        return Err(usage(format!("--key {} is not a file", key.display())));
    }
    outside_bazel_out(
        &parent,
        &app,
        env,
        &format!("--app {}", app.display()),
        "sign it to a path",
    )?;

    if bundle_id.is_empty() {
        return Err(usage("the launcher names no bundle_id"));
    }
    let name = app
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let app_id = value(
        tools,
        "CFBundleIdentifier",
        &app.join("Contents/Info.plist"),
    )
    .ok_or_else(|| usage(format!("{name} names no CFBundleIdentifier")))?;
    if &app_id != bundle_id {
        return Err(usage(format!(
            "{name} is {app_id}, not the declared {bundle_id}"
        )));
    }
    let details = describe(tools, &["-dv"], &app)
        .ok_or_else(|| usage(format!("{name} is not signed; run apple_macos_sign first")))?;
    if details
        .lines()
        .any(|line| line.starts_with("Signature=adhoc"))
    {
        return Err(usage(format!(
            "{name} carries an ad hoc signature; run apple_macos_sign first"
        )));
    }
    if !hardened(&details) {
        return Err(usage(format!(
            "{name} is not signed with the hardened runtime; run apple_macos_sign first"
        )));
    }

    let work = Scratch::new(env, "notarize")?;
    let zip = work
        .path()
        .join(format!("{}.zip", name.trim_end_matches(".app")));
    let zipped = tools.tool(
        DITTO,
        &["-c", "-k", "--keepParent", &text(&app), &text(&zip)],
    );
    if !zipped.success() {
        return Err(usage(format!("cannot zip {}", app.display())));
    }
    let key_text = text(&key);
    let auth: Vec<&str> = if profile.is_empty() {
        vec!["--key", &key_text, "--key-id", key_id, "--issuer", issuer]
    } else {
        vec!["--keychain-profile", profile]
    };

    let mut submit = vec!["notarytool", "submit"];
    let zip_text = text(&zip);
    submit.push(&zip_text);
    submit.extend(&auth);
    submit.extend(["--wait", "--timeout", timeout, "--output-format", "json"]);
    let submitted = tools.xcrun(&submit);
    let json = work.path().join("submit.json");
    let _ = std::fs::write(&json, &submitted.stdout);
    let id = value(tools, "id", &json).unwrap_or_default();
    let status = value(tools, "status", &json).unwrap_or_default();
    if id.is_empty() {
        eprint!("{}", submitted.stderr);
        if submitted.stderr.contains("NSURLErrorDomain")
            || submitted.stderr.contains("appears to be offline")
        {
            return Err(Failure {
                code: 3,
                message: "the Apple notary service is not reachable; retry when online".to_owned(),
            });
        }
        let code = match submitted.code {
            Some(0) | None => 1,
            code => exit_code(code),
        };
        return Err(Failure {
            code,
            message: "notarytool submit returned no submission id".to_owned(),
        });
    }

    let log = PathBuf::from(format!("{}.notary.json", app.display()));
    let _ = std::fs::remove_file(&log);
    let log_text = text(&log);
    let mut fetch = vec!["notarytool", "log", &id, &log_text];
    fetch.extend(&auth);
    if !tools.xcrun(&fetch).success() {
        eprintln!("{PREFIX}: cannot fetch the notary log of submission {id}");
    }

    match status.as_str() {
        "Accepted" => {}
        "In Progress" => {
            return Err(Failure {
                code: 3,
                message: format!(
                    "submission {id} is still In Progress after --timeout {timeout}; check it with xcrun notarytool info {id}"
                ),
            });
        }
        "Invalid" | "Rejected" => {
            return Err(reject(format!(
                "submission {id} of {name} is {status}; see the notary log {log_text}"
            )));
        }
        _ => {
            return Err(reject(format!(
                "submission {id} of {name} ended with status '{status}'; see the notary log {log_text}"
            )));
        }
    }

    let app_text = text(&app);
    let staple = tools.xcrun(&["stapler", "staple", &app_text]);
    if !staple.success() {
        eprint!("{}", combined(&staple));
        return Err(reject(format!(
            "cannot staple the notarization ticket to {name}"
        )));
    }
    let validate = tools.xcrun(&["stapler", "validate", &app_text]);
    if !validate.success() {
        eprint!("{}", combined(&validate));
        return Err(reject(format!("stapled {name} fails stapler validate")));
    }
    let assess = tools.tool(SPCTL, &["--assess", "--type", "execute", "-vv", &app_text]);
    if !assess.success() {
        eprint!("{}", combined(&assess));
        return Err(reject(format!(
            "stapled {name} fails spctl --assess --type execute"
        )));
    }
    println!(
        "{PREFIX}: notarized and stapled {app_text} (submission {id}); notary log at {log_text}"
    );
    Ok(0)
}
