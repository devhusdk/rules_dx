//! Signs a built macOS app with a Developer ID.

use std::path::{Path, PathBuf};

use crate::signing::{
    Flags, Keychain, absolute, caller, codesign, debuggable, descendants, describe, detail,
    entries, hardened, is_link, keys, macho, output_path, private_files, read_identity, text,
    value,
};
use crate::util::{Env, Failure, Outcome, Scratch, missing, reject, usage};
use crate::xcode::{CODESIGN, DITTO, PLUTIL, SECURITY, Tools};

pub const PREFIX: &str = "dx-apple-sign";

const HELP: &str = "usage: bazel run <sign> -- (--certificate <identity.p12> --password-file <file> | --identity <SHA-1 or name> [--keychain <keychain>]) --out <App.app>";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Framework,
    Nested,
    Main,
}

struct Plan<'a> {
    tools: &'a Tools<'a>,
    app: PathBuf,
    name: String,
    steps: Vec<(Step, PathBuf)>,
    files: Vec<PathBuf>,
    prefixes: Vec<String>,
}

const NESTED_DIRS: [&str; 7] = [
    "PlugIns",
    "XPCServices",
    "Helpers",
    "Library/LoginItems",
    "Library/SystemExtensions",
    "Library/QuickLook",
    "Library/Spotlight",
];

impl Plan<'_> {
    fn inside(&self, path: &Path) -> String {
        if path == self.app {
            self.name.clone()
        } else {
            text(path.strip_prefix(&self.app).unwrap_or(path))
        }
    }

    fn refuse_debuggable(&self, path: &Path) -> Result<(), Failure> {
        let out = self.tools.tool(
            CODESIGN,
            &["-d", "--entitlements", "-", "--xml", &text(path)],
        );
        if out.success() && debuggable(&out.stdout) {
            return Err(usage(format!(
                "{} sets entitlement 'com.apple.security.get-task-allow'; notarization rejects debuggable code",
                self.inside(path)
            )));
        }
        Ok(())
    }

    fn nested(&mut self, item: &Path) -> Result<(), Failure> {
        if item.is_dir() {
            return self.bundle(item, Step::Nested);
        }
        self.refuse_debuggable(item)?;
        self.files.push(item.to_path_buf());
        self.steps.push((Step::Nested, item.to_path_buf()));
        Ok(())
    }

    fn bundle(&mut self, bundle: &Path, kind: Step) -> Result<(), Failure> {
        let contents = bundle.join("Contents");
        let info = contents.join("Info.plist");
        if !info.is_file() {
            return Err(usage(format!(
                "{} carries {} without Contents/Info.plist; cannot sign it",
                self.name,
                self.inside(bundle)
            )));
        }
        let main = value(self.tools, "CFBundleExecutable", &info).ok_or_else(|| {
            usage(format!(
                "{} names no CFBundleExecutable",
                self.inside(bundle)
            ))
        })?;
        if kind == Step::Nested {
            self.refuse_debuggable(bundle)?;
        }
        let frameworks = contents.join("Frameworks");
        if frameworks.is_dir() {
            for item in entries(&frameworks) {
                if is_link(&item) {
                    continue;
                }
                if item.is_dir() {
                    if let Some(inner) =
                        descendants(&item).into_iter().find(|path| inner_code(path))
                    {
                        return Err(usage(format!(
                            "{} carries nested code at {}; signing code nested in a framework is not supported yet",
                            self.name,
                            self.inside(&inner)
                        )));
                    }
                    self.prefixes.push(format!("{}/", item.display()));
                } else {
                    self.files.push(item.clone());
                }
                self.steps.push((Step::Framework, item));
            }
        }
        for dir in NESTED_DIRS {
            let dir = contents.join(dir);
            if !dir.is_dir() {
                continue;
            }
            for item in entries(&dir) {
                if !is_link(&item) {
                    self.nested(&item)?;
                }
            }
        }
        let macos = contents.join("MacOS");
        if macos.is_dir() {
            for item in entries(&macos) {
                if item != macos.join(&main) && macho(&item) {
                    self.nested(&item)?;
                }
            }
        }
        self.files.push(macos.join(&main));
        self.steps.push((kind, bundle.to_path_buf()));
        Ok(())
    }

    fn covered(&self, file: &Path) -> bool {
        self.files.iter().any(|known| known == file)
            || self
                .prefixes
                .iter()
                .any(|prefix| text(file).starts_with(prefix.as_str()))
    }
}

fn inner_code(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.ends_with(".app")
        || name.ends_with(".appex")
        || name.ends_with(".framework")
        || name.ends_with(".xpc")
        || ["Helpers", "XPCServices", "PlugIns"].contains(&name.as_str())
}

fn app_in(dir: &Path, archive: &Path) -> Result<PathBuf, Failure> {
    let apps: Vec<PathBuf> = entries(dir)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|end| end == "app") && path.is_dir())
        .collect();
    match apps.len() {
        0 => Err(usage(format!("{} holds no .app bundle", archive.display()))),
        1 => Ok(apps.into_iter().next().unwrap_or_default()),
        _ => Err(usage(format!(
            "{} holds more than one .app bundle",
            archive.display()
        ))),
    }
}

fn identity_sha1(
    tools: &Tools,
    work: &Path,
    identity: &str,
    keychain: &str,
) -> Result<String, Failure> {
    let mut args = vec!["find-identity", "-v", "-p", "codesigning"];
    if !keychain.is_empty() {
        args.push(keychain);
    }
    let listed = tools.tool(SECURITY, &args);
    if !listed.success() {
        return Err(usage(if keychain.is_empty() {
            "cannot list the code signing identities".to_owned()
        } else {
            format!("cannot list the code signing identities in {keychain}")
        }));
    }
    let _ = work;
    let mut matches = std::collections::BTreeSet::new();
    for line in listed.stdout.lines() {
        let mut fields = line.split_whitespace();
        let (Some(first), Some(hash)) = (fields.next(), fields.next()) else {
            continue;
        };
        if !first.ends_with(')')
            || !first[..first.len() - 1].chars().all(|c| c.is_ascii_digit())
            || first.len() < 2
        {
            continue;
        }
        if fields.next().is_none() {
            continue;
        }
        let label = line.split_once('"').map_or("", |(_, rest)| {
            rest.rsplit_once('"').map_or(rest, |(label, _)| label)
        });
        if hash.eq_ignore_ascii_case(identity) || label.contains(identity) {
            matches.insert(hash.to_uppercase());
        }
    }
    match matches.len() {
        0 => Err(usage(format!(
            "no valid code signing identity matches --identity {identity}"
        ))),
        1 => Ok(matches.into_iter().next().unwrap_or_default()),
        _ => Err(usage(format!(
            "--identity {identity} matches more than one code signing identity; pass its SHA-1"
        ))),
    }
}

/// Signs the app `args[1]` for bundle id `args[0]` with entitlements `args[2]`; the flags follow.
pub fn run(tools: &Tools, args: &[String], env: Env) -> Outcome {
    private_files();
    let [bundle_id, archive, declared, flag_args @ ..] = args else {
        return Err(usage("run the signing launcher with bazel run"));
    };
    let flags = Flags::parse(
        flag_args,
        &[
            "certificate",
            "password-file",
            "identity",
            "keychain",
            "out",
        ],
        HELP,
    )?;
    let (mut cert, mut password_file) = (flags.get("certificate"), flags.get("password-file"));
    let (mut identity, mut user_keychain) = (
        flags.get("identity").to_owned(),
        flags.get("keychain").to_owned(),
    );
    let preset = [
        cert,
        password_file,
        identity.as_str(),
        user_keychain.as_str(),
    ];
    if preset.iter().all(|value| value.is_empty()) {
        identity = env("DX_APPLE_SIGN_IDENTITY").unwrap_or_default();
        user_keychain = env("DX_APPLE_KEYCHAIN").unwrap_or_default();
    }
    if !identity.is_empty() {
        if !cert.is_empty() || !password_file.is_empty() {
            return Err(usage(format!(
                "pass --identity or --certificate with --password-file, not both; {HELP}"
            )));
        }
        cert = "";
        password_file = "";
    } else {
        if !user_keychain.is_empty() {
            return Err(usage(format!(
                "--keychain only goes with --identity; {HELP}"
            )));
        }
        if cert.is_empty() {
            return Err(usage(format!(
                "--certificate names the Developer ID identity .p12 file, or --identity names one in a keychain; {HELP}"
            )));
        }
        if password_file.is_empty() {
            return Err(usage(format!(
                "--password-file names the file holding the .p12 password; {HELP}"
            )));
        }
    }
    if flags.get("out").is_empty() {
        return Err(usage(format!("--out names the .app to write; {HELP}")));
    }
    let caller = caller(env);
    let out = absolute(flags.get("out"), &caller);
    if !identity.is_empty() {
        if user_keychain.contains('/') {
            let path = absolute(&user_keychain, &caller);
            if !path.is_file() {
                return Err(usage(format!(
                    "--keychain {} is not a file",
                    path.display()
                )));
            }
            user_keychain = text(&path);
        } else if !user_keychain.is_empty() && caller.join(&user_keychain).is_file() {
            user_keychain = text(&caller.join(&user_keychain));
        }
    }
    let cert = absolute(cert, &caller);
    let password_file = absolute(password_file, &caller);
    if identity.is_empty() {
        if !cert.is_file() {
            return Err(usage(format!(
                "--certificate {} is not a file",
                cert.display()
            )));
        }
        if !password_file.is_file() {
            return Err(usage(format!(
                "--password-file {} is not a file",
                password_file.display()
            )));
        }
    }
    let out = output_path(&out, env, ".app", "an .app bundle")?;

    if bundle_id.is_empty() {
        return Err(usage("the launcher names no bundle_id"));
    }
    let archive = PathBuf::from(archive);
    if !archive.exists() {
        return Err(usage(format!(
            "built app archive {} is missing",
            archive.display()
        )));
    }
    let declared = PathBuf::from(declared);
    if !declared.as_os_str().is_empty() && !declared.is_file() {
        return Err(usage(format!(
            "declared entitlements {} are missing",
            declared.display()
        )));
    }

    let work = Scratch::new(env, "sign")?;
    let unpacked = work.path().join("unpacked");
    std::fs::create_dir_all(&unpacked)
        .map_err(|e| usage(format!("cannot create {}: {e}", unpacked.display())))?;
    let archive_text = text(&archive);
    let unpack = if archive_text.ends_with(".app") {
        if !archive.is_dir() {
            return Err(usage(format!(
                "built app {} is not a bundle directory",
                archive.display()
            )));
        }
        let name = archive.file_name().unwrap_or_default();
        let copy = unpacked.join(name);
        (
            tools.tool(DITTO, &[&archive_text, &text(&copy)]),
            format!("cannot copy {}", archive.display()),
        )
    } else {
        (
            tools.tool(DITTO, &["-x", "-k", &archive_text, &text(&unpacked)]),
            format!("cannot unpack {}", archive.display()),
        )
    };
    if !unpack.0.success() {
        return Err(usage(unpack.1));
    }
    let app = app_in(&unpacked, &archive)?;
    let name = app
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let mut plan = Plan {
        tools,
        app: app.clone(),
        name: name.clone(),
        steps: Vec::new(),
        files: Vec::new(),
        prefixes: Vec::new(),
    };
    plan.bundle(&app, Step::Main)?;
    for file in descendants(&app) {
        if macho(&file) && !plan.covered(&file) {
            return Err(usage(format!(
                "{name} carries code at {} outside the places this launcher signs; move it into Contents/Frameworks, PlugIns, XPCServices, Helpers, MacOS, or Library/LoginItems, SystemExtensions, QuickLook or Spotlight",
                plan.inside(&file)
            )));
        }
    }

    let mut entitlements: Option<PathBuf> = None;
    let mut entitlement_keys = String::new();
    if !declared.as_os_str().is_empty() {
        let converted = work.path().join("entitlements.plist");
        let read = tools.tool(
            PLUTIL,
            &[
                "-convert",
                "xml1",
                "-o",
                &text(&converted),
                &text(&declared),
            ],
        );
        if !read.success() {
            return Err(usage(format!(
                "cannot read the entitlements {}",
                declared.display()
            )));
        }
        let xml = std::fs::read_to_string(&converted).unwrap_or_default();
        entitlement_keys = keys(&xml).into_iter().collect::<Vec<_>>().join(",");
        if debuggable(&xml) {
            return Err(usage(
                "entitlement 'com.apple.security.get-task-allow' is true; notarization rejects debuggable apps",
            ));
        }
        entitlements = Some(converted);
    }

    let (sha1, _keychain, sign_keychain);
    if identity.is_empty() {
        let opened = read_identity(tools, work.path(), &cert, &password_file)?;
        let keychain = Keychain::open(tools, work.path(), &opened, &cert, "signing")?;
        sha1 = opened.sha1;
        sign_keychain = text(&keychain.path);
        _keychain = Some(keychain);
    } else {
        sha1 = identity_sha1(tools, work.path(), &identity, &user_keychain)?;
        sign_keychain = user_keychain.clone();
        _keychain = None;
    }

    for (step, target) in &plan.steps {
        let target = text(target);
        let mut sign = vec!["--force", "--options", "runtime", "--timestamp"];
        if !sign_keychain.is_empty() {
            sign.extend(["--keychain", &sign_keychain]);
        }
        sign.extend(["--sign", &sha1]);
        let entitlements_text = entitlements.as_ref().map(|path| text(path));
        match (step, &entitlements_text) {
            (Step::Nested, _) => sign.push("--preserve-metadata=entitlements"),
            (Step::Main, Some(path)) => sign.extend(["--entitlements", path]),
            _ => {}
        }
        sign.push(&target);
        codesign(tools, &sign)?;
    }

    let verify = tools.tool(CODESIGN, &["--verify", "--deep", "--strict", &text(&app)]);
    if !verify.success() {
        return Err(reject(format!(
            "signed {name} fails codesign --verify --deep --strict"
        )));
    }
    let details = describe(tools, &["-dvv"], &app)
        .ok_or_else(|| reject(format!("cannot read the signature of {name}")))?;
    let team = detail(&details, "TeamIdentifier=");
    let authority = detail(&details, "Authority=");
    let timestamp = detail(&details, "Timestamp=");
    if !hardened(&details) {
        return Err(reject(format!(
            "signed {name} carries no hardened runtime flag"
        )));
    }
    if authority.is_empty() {
        return Err(reject(format!(
            "signed {name} carries no signing authority"
        )));
    }
    if timestamp.is_empty() {
        return Err(reject(format!("signed {name} carries no secure timestamp")));
    }
    let executable = value(
        tools,
        "CFBundleExecutable",
        &app.join("Contents/Info.plist"),
    )
    .ok_or_else(|| reject(format!("{name} names no CFBundleExecutable")))?;
    let binary = text(&app.join("Contents/MacOS").join(&executable));
    let lipo = tools.xcrun(&["lipo", "-archs", &binary]);
    if lipo.code.is_none() {
        return Err(missing(format!(
            "cannot start xcrun: {}",
            lipo.stderr.trim()
        )));
    }
    if !lipo.success() {
        return Err(reject(format!("{executable} is not a Mach-O file")));
    }
    let archs = lipo.stdout.trim_end_matches('\n').to_owned();

    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_file(&out);
    let notes = PathBuf::from(format!("{}.txt", out.display()));
    let _ = std::fs::remove_file(&notes);
    if !tools.tool(DITTO, &[&text(&app), &text(&out)]).success() {
        return Err(usage(format!("cannot write {}", out.display())));
    }
    let report = format!(
        "bundle_id={bundle_id}\nteam={team}\nauthority={authority}\ncertificate_sha1={sha1}\ntimestamp={timestamp}\nentitlements={entitlement_keys}\narchs={archs}\nruntime=yes\n"
    );
    std::fs::write(&notes, report)
        .map_err(|e| usage(format!("cannot write {}: {e}", notes.display())))?;
    println!("{PREFIX}: wrote {} and {}", out.display(), notes.display());
    Ok(0)
}
