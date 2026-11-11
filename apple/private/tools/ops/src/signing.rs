//! What codesign, export and notarize share: flags, paths, plists, certificates and keychains.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::util::{Env, Failure, exit_code, usage};
use crate::xcode::{OPENSSL, PLUTIL, SECURITY, Tools};

/// Makes every file this operation creates private to the user.
pub fn private_files() {
    // SAFETY: umask has no failure mode and only changes the creation mask of this process.
    unsafe {
        libc::umask(0o077);
    }
}

/// The text of a path.
pub fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The flags of one invocation by name without the leading dashes.
pub struct Flags(HashMap<String, String>);

impl Flags {
    /// Reads `--name value` and `--name=value` for the `names` given.
    pub fn parse(args: &[String], names: &[&str], help: &str) -> Result<Flags, Failure> {
        let mut found = HashMap::new();
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            let Some(flag) = arg.strip_prefix("--") else {
                return Err(usage(format!("unexpected argument; {help}")));
            };
            let (name, inline) = match flag.split_once('=') {
                Some((name, value)) => (name, Some(value.to_owned())),
                None => (flag, None),
            };
            if !names.contains(&name) {
                return Err(usage(format!("unexpected argument; {help}")));
            }
            let value = match inline {
                Some(value) => value,
                None => rest
                    .next()
                    .cloned()
                    .ok_or_else(|| usage(format!("--{name} needs a value; {help}")))?,
            };
            found.insert(name.to_owned(), value);
        }
        Ok(Flags(found))
    }

    /// The value of `name`, empty when it is not given.
    pub fn get(&self, name: &str) -> &str {
        self.0.get(name).map_or("", String::as_str)
    }
}

/// The directory the user ran the program from.
pub fn caller(env: Env) -> PathBuf {
    env("BUILD_WORKING_DIRECTORY")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// `path` below `base` unless it is absolute.
pub fn absolute(path: &str, base: &Path) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// The checked location of the file or bundle `out`: its directory exists and is outside bazel-out.
pub fn output_path(out: &Path, env: Env, suffix: &str, what: &str) -> Result<PathBuf, Failure> {
    if !text(out).ends_with(suffix) {
        return Err(usage(format!("--out {} must name {what}", out.display())));
    }
    let parent = out.parent().unwrap_or_else(|| Path::new("."));
    let dir = parent.canonicalize().map_err(|_| {
        usage(format!(
            "--out directory {} does not exist",
            parent.display()
        ))
    })?;
    let out = dir.join(out.file_name().unwrap_or_default());
    outside_bazel_out(&dir, &out, env, &format!("--out {}", out.display()), what)?;
    Ok(out)
}

/// Fails when `dir` is inside the Bazel output base.
pub fn outside_bazel_out(
    dir: &Path,
    shown: &Path,
    env: Env,
    flag: &str,
    advice: &str,
) -> Result<(), Failure> {
    let Some(workspace) = env("BUILD_WORKSPACE_DIRECTORY") else {
        return Ok(());
    };
    let Ok(bazel_out) = Path::new(&workspace).join("bazel-out").canonicalize() else {
        return Ok(());
    };
    let whole = text(&bazel_out);
    let base = whole.split_once("/execroot/").map_or(whole.as_str(), |(base, _)| base);
    for root in [bazel_out.as_path(), Path::new(base)] {
        if dir.starts_with(root) {
            return Err(usage(format!(
                "{flag} is inside the Bazel output base; {advice} outside bazel-out ({})",
                shown.display()
            )));
        }
    }
    Ok(())
}

/// The sorted entries of `dir`.
pub fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|read| read.filter_map(Result::ok).map(|entry| entry.path()).collect())
        .unwrap_or_default();
    found.sort();
    found
}

/// Whether `path` is a symbolic link.
pub fn is_link(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|meta| meta.file_type().is_symlink())
}

/// Every file or directory below `dir`, without following links.
pub fn descendants(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in entries(dir) {
        let below = !is_link(&entry) && entry.is_dir();
        found.push(entry.clone());
        if below {
            found.extend(descendants(&entry));
        }
    }
    found
}

/// Whether `path` is a regular file that starts with a Mach-O or universal magic.
pub fn macho(path: &Path) -> bool {
    use std::io::Read;
    if is_link(path) || !path.is_file() {
        return false;
    }
    let mut magic = [0u8; 4];
    let read = std::fs::File::open(path).and_then(|mut file| file.read_exact(&mut magic));
    read.is_ok()
        && matches!(
            magic,
            [0xce, 0xfa, 0xed, 0xfe]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xce]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xca, 0xfe, 0xba, 0xbf]
        )
}

/// The value of `key` in the plist `file`.
pub fn value(tools: &Tools, key: &str, file: &Path) -> Option<String> {
    let out = tools.tool(PLUTIL, &["-extract", key, "raw", "-o", "-", &text(file)]);
    out.success().then(|| out.stdout.trim_end_matches('\n').to_owned())
}

/// The top-level keys of an XML plist.
pub fn keys(plist: &str) -> BTreeSet<String> {
    let mut depth = 0i32;
    let mut found = BTreeSet::new();
    for line in plist.lines() {
        if line.contains("<dict>") {
            depth += 1;
        } else if line.contains("</dict>") {
            depth -= 1;
        } else if depth == 1 && line.contains("<key>") {
            let after = line.rsplit_once("<key>").map_or("", |(_, after)| after);
            found.insert(after.split_once("</key>").map_or(after, |(key, _)| key).to_owned());
        }
    }
    found
}

/// Whether the entitlements plist sets `com.apple.security.get-task-allow` to true.
pub fn debuggable(plist: &str) -> bool {
    let squeezed: String = plist.chars().filter(|c| !" \t\r\n".contains(*c)).collect();
    squeezed.contains("<key>com.apple.security.get-task-allow</key><true/>")
}

/// The first value of `name=` in codesign details.
pub fn detail(details: &str, name: &str) -> String {
    details
        .lines()
        .find_map(|line| line.strip_prefix(name))
        .unwrap_or("")
        .to_owned()
}

/// Whether the codesign details show the hardened runtime flag.
pub fn hardened(details: &str) -> bool {
    details.lines().any(|line| {
        line.starts_with("CodeDirectory ")
            && line.match_indices("flags=").any(|(at, _)| {
                line[at + 6..].split(' ').next().is_some_and(|flags| flags.contains("runtime"))
            })
    })
}

/// Reads the base64 `encoded` text.
pub fn base64_decode(encoded: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let (mut bits, mut count) = (0u32, 0);
    for ch in encoded.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        let digit = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        bits = bits << 6 | digit;
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push((bits >> count & 0xff) as u8);
        }
    }
    Some(bytes)
}

/// The SHA-1 fingerprint of the certificate in `file`, upper case without colons.
pub fn fingerprint(tools: &Tools, file: &Path, form: &str) -> Option<String> {
    let out = tools.tool(
        OPENSSL,
        &["x509", "-in", &text(file), "-inform", form, "-noout", "-fingerprint", "-sha1"],
    );
    let line = out.stdout.lines().next()?;
    let print: String = line
        .rsplit_once('=')?
        .1
        .chars()
        .filter(|c| *c != ':')
        .collect::<String>()
        .to_uppercase();
    (out.success() && !print.is_empty()).then_some(print)
}

/// A random hex string from openssl.
fn random(tools: &Tools) -> Result<String, Failure> {
    let out = tools.tool(OPENSSL, &["rand", "-hex", "24"]);
    let secret = out.stdout.trim().to_owned();
    if out.success() && !secret.is_empty() {
        Ok(secret)
    } else {
        Err(usage("cannot create a random password"))
    }
}

/// A signing identity re-encrypted for `security import`.
pub struct Identity {
    pub p12: PathBuf,
    pub password: String,
    pub sha1: String,
}

/// Reads the .p12 `cert` with the password in `password_file`.
pub fn read_identity(
    tools: &Tools,
    work: &Path,
    cert: &Path,
    password_file: &Path,
) -> Result<Identity, Failure> {
    let unreadable = || usage(format!("cannot read {} with the password in --password-file", cert.display()));
    let source = format!("file:{}", password_file.display());
    let pem = work.join("identity.pem");
    let password = random(tools)?;
    let transfer = work.join("transfer.pass");
    std::fs::write(&transfer, &password).map_err(|_| unreadable())?;
    let read = tools.tool(
        OPENSSL,
        &["pkcs12", "-in", &text(cert), "-passin", &source, "-nodes", "-out", &text(&pem)],
    );
    let p12 = work.join("identity.p12");
    let exported = read.success()
        && tools
            .tool(
                OPENSSL,
                &[
                    "pkcs12", "-export", "-in", &text(&pem),
                    "-passout", &format!("file:{}", transfer.display()),
                    "-keypbe", "PBE-SHA1-3DES", "-certpbe", "PBE-SHA1-3DES", "-macalg", "sha1",
                    "-out", &text(&p12),
                ],
            )
            .success();
    let _ = std::fs::remove_file(&pem);
    if !exported || std::fs::metadata(&p12).map_or(true, |meta| meta.len() == 0) {
        return Err(unreadable());
    }
    let certs = work.join("identity-cert.pem");
    let listed = tools.tool(
        OPENSSL,
        &["pkcs12", "-in", &text(cert), "-passin", &source, "-nokeys", "-clcerts", "-out", &text(&certs)],
    );
    let sha1 = if listed.success() { fingerprint(tools, &certs, "PEM") } else { None };
    let sha1 = sha1.ok_or_else(|| usage(format!("{} holds no signing certificate", cert.display())))?;
    Ok(Identity { p12, password, sha1 })
}

/// A throwaway keychain on the user's search list, removed when dropped.
pub struct Keychain<'a> {
    tools: &'a Tools<'a>,
    pub path: PathBuf,
    saved: Option<Vec<String>>,
}

fn search_list(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(|line| line.trim().trim_matches('"').to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

impl<'a> Keychain<'a> {
    /// Imports `identity` into a new keychain called `label` and searches it first.
    pub fn open(
        tools: &'a Tools<'a>,
        work: &Path,
        identity: &Identity,
        cert: &Path,
        label: &str,
    ) -> Result<Keychain<'a>, Failure> {
        let listing = tools.tool(SECURITY, &["list-keychains", "-d", "user"]);
        if !listing.success() {
            return Err(usage("cannot list the user keychains"));
        }
        let saved = search_list(&listing.stdout);
        let secret = random(tools)?;
        let path = work.join(format!("{label}.keychain-db"));
        let mut keychain = Keychain { tools, path, saved: None };
        let at = text(&keychain.path);
        let p12 = text(&identity.p12);
        let steps: [(Vec<&str>, String); 5] = [
            (vec!["create-keychain", "-p", &secret, &at], format!("cannot create the {label} keychain")),
            (vec!["set-keychain-settings", &at], format!("cannot configure the {label} keychain")),
            (vec!["unlock-keychain", "-p", &secret, &at], format!("cannot unlock the {label} keychain")),
            (
                vec![
                    "import", &p12, "-k", &at, "-f", "pkcs12",
                    "-P", &identity.password, "-T", crate::xcode::CODESIGN,
                ],
                format!("cannot import {} into the {label} keychain", cert.display()),
            ),
            (
                vec!["set-key-partition-list", "-S", "apple-tool:,apple:", "-s", "-k", &secret, &at],
                format!("cannot grant codesign access to the {label} keychain"),
            ),
        ];
        for (args, message) in steps {
            if !tools.tool(SECURITY, &args).success() {
                return Err(usage(message));
            }
        }
        keychain.saved = Some(saved.clone());
        let mut list = vec!["list-keychains", "-d", "user", "-s", &at];
        list.extend(saved.iter().map(String::as_str));
        if !tools.tool(SECURITY, &list).success() {
            return Err(usage(format!("cannot add the {label} keychain to the search list")));
        }
        let found = tools.tool(SECURITY, &["find-identity", "-v", "-p", "codesigning", &at]);
        if !found.success() || !found.stdout.contains(&identity.sha1) {
            return Err(usage(format!(
                "{} holds no valid code signing identity {}",
                cert.display(),
                identity.sha1
            )));
        }
        Ok(keychain)
    }
}

impl Drop for Keychain<'_> {
    fn drop(&mut self) {
        if let Some(saved) = &self.saved {
            let mut list = vec!["list-keychains", "-d", "user", "-s"];
            list.extend(saved.iter().map(String::as_str));
            let _ = self.tools.tool(SECURITY, &list);
        }
        let _ = self.tools.tool(SECURITY, &["delete-keychain", &text(&self.path)]);
    }
}

/// Runs codesign with `args`; prints its errors and maps a missing timestamp service to exit code 3.
pub fn codesign(tools: &Tools, args: &[&str]) -> Result<(), Failure> {
    let out = tools.tool(crate::xcode::CODESIGN, args);
    if out.success() {
        return Ok(());
    }
    eprint!("{}", out.stderr);
    if out.stderr.contains("timestamp service is not available") {
        return Err(Failure {
            code: 3,
            message: "the Apple timestamp service is not reachable; retry when online".to_owned(),
        });
    }
    Err(Failure { code: exit_code(out.code), message: "codesign failed".to_owned() })
}

/// The combined output of codesign when it describes `target`.
pub fn describe(tools: &Tools, flags: &[&str], target: &Path) -> Option<String> {
    let mut args = flags.to_vec();
    let at = text(target);
    args.push(&at);
    let out = tools.tool(crate::xcode::CODESIGN, &args);
    out.success().then(|| format!("{}{}", out.stdout, out.stderr))
}
