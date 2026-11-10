//! Signs one APK with apksigner or one App Bundle with jarsigner.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::{
    Captured, Failure, Outcome, Scratch, caller_path, capture, missing, passes, reject, runnable,
    usage,
};

pub const PREFIX: &str = "dx-android-sign";

const SCHEMES: [&str; 4] = ["v1", "v2", "v3", "v4"];

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

/// The value of `--<flag>=` among `args`; the last one wins.
pub fn flag(args: &[String], flag: &str) -> String {
    args.iter()
        .rev()
        .find_map(|arg| arg.strip_prefix("--")?.strip_prefix(flag)?.strip_prefix('='))
        .unwrap_or_default()
        .to_owned()
}

/// A launcher setting: the environment variable when set, else the launcher flag.
fn setting(args: &[String], name: &str, env: &str) -> String {
    Some(var(env))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| flag(args, name))
}

fn need(name: &str, hint: &str) -> Result<String, Failure> {
    let value = var(name);
    if value.is_empty() {
        return Err(usage(format!("{name} is not set: {hint}")));
    }
    Ok(value)
}

/// The key the user names: keystore, password files and alias.
struct Key {
    keystore: PathBuf,
    store_pass: PathBuf,
    key_pass: Option<PathBuf>,
    alias: String,
}

fn key(keystore_hint: &str) -> Result<Key, Failure> {
    let keystore = need("DX_SIGN_KEYSTORE", keystore_hint)?;
    let store_pass = need(
        "DX_SIGN_STORE_PASS_FILE",
        "name a file holding the keystore password",
    )?;
    let key_pass = Some(var("DX_SIGN_KEY_PASS_FILE")).filter(|value| !value.is_empty());
    Ok(Key {
        keystore: caller_path(&keystore),
        store_pass: caller_path(&store_pass),
        key_pass: key_pass.map(|value| caller_path(&value)),
        alias: var("DX_SIGN_KEY_ALIAS"),
    })
}

fn key_files(key: &Key) -> Result<(), Failure> {
    if !key.keystore.is_file() {
        return Err(usage(format!("no keystore at {}", key.keystore.display())));
    }
    if !key.store_pass.is_file() {
        return Err(usage(format!(
            "no password file at {}",
            key.store_pass.display()
        )));
    }
    match &key.key_pass {
        Some(file) if !file.is_file() => Err(usage(format!(
            "no key password file at {}",
            file.display()
        ))),
        _ => Ok(()),
    }
}

fn file_arg(prefix: &str, file: &Path) -> String {
    format!("{prefix}{}", file.display())
}

/// Whether both paths name one file.
fn same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn scratch(out: &Path) -> Result<Scratch, Failure> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| usage(format!("cannot create {}: {error}", parent.display())))?;
    }
    Scratch::beside(out).map_err(|error| usage(format!("cannot create a scratch directory: {error}")))
}

fn publish(from: &Path, to: &Path) -> Result<(), Failure> {
    std::fs::rename(from, to)
        .map_err(|error| reject(format!("cannot write {}: {error}", to.display())))?;
    println!("{PREFIX}: wrote {}", to.display());
    Ok(())
}

/// Whether the schemes ask for a v4 signature; v4 alone is an error.
pub fn wants_v4(schemes: &[&str]) -> Result<bool, &'static str> {
    if !schemes.contains(&"v4") {
        return Ok(false);
    }
    if schemes.contains(&"v2") || schemes.contains(&"v3") {
        return Ok(true);
    }
    Err("scheme v4 needs v2 or v3")
}

/// The apksigner flags that turn each scheme on or off.
pub fn scheme_flags(schemes: &[&str]) -> Vec<String> {
    SCHEMES
        .iter()
        .flat_map(|scheme| {
            [
                format!("--{scheme}-signing-enabled"),
                schemes.contains(scheme).to_string(),
            ]
        })
        .collect()
}

pub fn run_apk(args: &[String]) -> Outcome {
    let apk = setting(args, "apk", "DX_SIGN_APK");
    if apk.is_empty() {
        return Err(usage(
            "DX_SIGN_APK is not set: the unsigned APK comes from the launcher",
        ));
    }
    let key = key("name the release keystore file")?;
    let out = caller_path(&need("DX_SIGN_OUT", "name the signed APK path to write")?);
    let zipalign = PathBuf::from(setting(args, "zipalign", "DX_SIGN_ZIPALIGN"));
    let jar = PathBuf::from(setting(args, "apksigner-jar", "DX_SIGN_APKSIGNER_JAR"));
    let java = PathBuf::from(setting(args, "java", "DX_SIGN_JAVA"));
    let mut build_tools = setting(args, "build-tools", "DX_SIGN_BUILD_TOOLS");
    if build_tools.is_empty() {
        build_tools = "unknown".to_owned();
    }
    let fix = format!(
        "build-tools {build_tools} is missing; configure the android_sdk extension to acquire build-tools"
    );
    if !runnable(&zipalign) {
        return Err(missing(format!("no zipalign: {fix}")));
    }
    if !jar.is_file() {
        return Err(missing(format!("no apksigner: {fix}")));
    }
    if !runnable(&java) {
        return Err(missing(
            "no java runtime: select one with --java_runtime_version",
        ));
    }
    let shown = apk.clone();
    let apk = PathBuf::from(apk);
    if !apk.is_file() {
        return Err(usage(format!("no APK at {shown}")));
    }
    key_files(&key)?;
    if same_file(&apk, &out) {
        return Err(usage("DX_SIGN_OUT must not overwrite the input APK"));
    }
    let mut chosen = setting(args, "schemes", "DX_SIGN_SCHEMES");
    if chosen.is_empty() {
        chosen = "v1,v2,v3".to_owned();
    }
    let schemes: Vec<&str> = chosen.split(',').collect();
    let v4 = wants_v4(&schemes).map_err(usage)?;
    let min_sdk = setting(args, "min-sdk", "DX_SIGN_MIN_SDK");
    let work = scratch(&out)?;
    let aligned = work.path().join("aligned.apk");
    let signed = work.path().join("signed.apk");
    let idsig = work.path().join("signed.apk.idsig");
    if !passes(
        Command::new(&zipalign)
            .args(["-f", "-p", "4"])
            .arg(&apk)
            .arg(&aligned),
    ) {
        return Err(reject(format!("zipalign rejected {shown}")));
    }
    let apksigner = || {
        let mut command = Command::new(&java);
        command.arg("-Xmx1024M").arg("-jar").arg(&jar);
        command
    };
    let mut sign = apksigner();
    sign.arg("sign")
        .arg("--ks")
        .arg(&key.keystore)
        .arg("--ks-pass")
        .arg(file_arg("file:", &key.store_pass));
    if !key.alias.is_empty() {
        sign.arg("--ks-key-alias").arg(&key.alias);
    }
    if let Some(file) = &key.key_pass {
        sign.arg("--key-pass").arg(file_arg("file:", file));
    }
    if !min_sdk.is_empty() {
        sign.arg("--min-sdk-version").arg(&min_sdk);
    }
    sign.args(scheme_flags(&schemes))
        .arg("--out")
        .arg(&signed)
        .arg(&aligned);
    if !passes(&mut sign) {
        return Err(reject(format!("apksigner could not sign {shown}")));
    }
    let mut verify = apksigner();
    verify.arg("verify");
    if !min_sdk.is_empty() {
        verify.arg("--min-sdk-version").arg(&min_sdk);
    }
    if v4 {
        if !idsig.is_file() {
            return Err(reject("apksigner wrote no v4 signature"));
        }
        verify.arg("--v4-signature-file").arg(&idsig);
    }
    if !passes(verify.arg(&signed)) {
        return Err(reject("signed APK failed verification"));
    }
    let mut out_idsig = out.clone().into_os_string();
    out_idsig.push(".idsig");
    let out_idsig = PathBuf::from(out_idsig);
    let _ = std::fs::remove_file(&out_idsig);
    publish(&signed, &out)?;
    if v4 {
        publish(&idsig, &out_idsig)?;
    }
    Ok(0)
}

/// The jarsigner signature algorithm for the key `keytool -list -v` describes.
pub fn signature_algorithm(listing: &str) -> Option<&'static str> {
    let line = listing
        .lines()
        .find(|line| line.starts_with("Subject Public Key Algorithm:"))?;
    if line.contains(" RSA key") {
        return Some("SHA256withRSA");
    }
    if line.contains(" EC ") || line.contains(" EC key") {
        return Some("SHA256withECDSA");
    }
    None
}

/// Why `jarsigner -verify -strict` output does not prove a signed bundle, if it does not.
pub fn verify_error(ran: &Captured) -> Option<&'static str> {
    let plain = "signed App Bundle failed verification";
    match ran.code {
        Some(0) => {}
        Some(4) => {
            if !ran.text.contains("self-signed") {
                return Some(plain);
            }
            if ["has expired", "is expired", "not yet valid"]
                .iter()
                .any(|sign| ran.text.contains(sign))
            {
                return Some(
                    "signed App Bundle failed verification: the certificate is expired or not yet valid",
                );
            }
        }
        _ => return Some(plain),
    }
    (!ran.line_starts("jar verified")).then_some(plain)
}

/// The JDK directory: `DX_SIGN_JAVABASE` when set, else two levels above the launcher's java.
fn java_home(args: &[String]) -> PathBuf {
    let home = var("DX_SIGN_JAVABASE");
    if !home.is_empty() {
        return PathBuf::from(home);
    }
    let java = PathBuf::from(flag(args, "java"));
    java.parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

pub fn run_aab(args: &[String]) -> Outcome {
    let aab = setting(args, "aab", "DX_SIGN_AAB");
    if aab.is_empty() {
        return Err(usage(
            "DX_SIGN_AAB is not set: the unsigned App Bundle comes from the launcher",
        ));
    }
    let key = key("name the upload keystore file")?;
    need("DX_SIGN_KEY_ALIAS", "name the upload key alias")?;
    let out = caller_path(&need(
        "DX_SIGN_OUT",
        "name the signed App Bundle path to write",
    )?);
    let bundletool = PathBuf::from(setting(args, "bundletool", "DX_SIGN_BUNDLETOOL"));
    let home = java_home(args);
    if !runnable(&bundletool) {
        return Err(missing(
            "no bundletool: the launcher supplies //android/private/packaging:bundletool",
        ));
    }
    let end = std::env::consts::EXE_SUFFIX;
    let jarsigner = home.join("bin").join(format!("jarsigner{end}"));
    let keytool = home.join("bin").join(format!("keytool{end}"));
    if !runnable(&jarsigner) || !runnable(&keytool) {
        return Err(missing(
            "no jarsigner: select a full JDK with --java_runtime_version",
        ));
    }
    let english = |tool: &Path| {
        let mut command = Command::new(tool);
        command.args(["-J-Duser.language=en", "-J-Duser.country=US"]);
        command
    };
    let shown = aab.clone();
    let aab = PathBuf::from(aab);
    if !aab.is_file() {
        return Err(usage(format!("no App Bundle at {shown}")));
    }
    key_files(&key)?;
    if same_file(&aab, &out) {
        return Err(usage(
            "DX_SIGN_OUT must not overwrite the input App Bundle",
        ));
    }
    let work = scratch(&out)?;
    if capture(english(&jarsigner).arg("-verify").arg(&aab)).line_starts("jar verified") {
        return Err(reject(format!("{shown} is already signed")));
    }
    let validated = capture(
        Command::new(&bundletool)
            .arg(format!(
                "--jvm_flag=-Djava.io.tmpdir={}",
                work.path().display()
            ))
            .arg("validate")
            .arg(file_arg("--bundle=", &aab)),
    );
    if !validated.success() {
        return Err(reject(format!(
            "bundletool validate rejects {shown}: {}",
            validated.first_reason()
        )));
    }
    let listed = capture(
        english(&keytool)
            .args(["-list", "-v", "-keystore"])
            .arg(&key.keystore)
            .arg("-storepass:file")
            .arg(&key.store_pass)
            .arg("-alias")
            .arg(&key.alias),
    );
    if !listed.success() {
        return Err(reject(format!(
            "jarsigner could not sign {shown}: keystore or alias rejected"
        )));
    }
    let algorithm = signature_algorithm(&listed.text).ok_or_else(|| {
        reject(format!(
            "jarsigner could not sign {shown}: the upload key must be RSA or EC"
        ))
    })?;
    let input = work.path().join("in.aab");
    let signed = work.path().join("signed.aab");
    std::fs::copy(&aab, &input)
        .map_err(|error| reject(format!("cannot copy {shown}: {error}")))?;
    let mut sign = english(&jarsigner);
    sign.arg("-keystore")
        .arg(&key.keystore)
        .arg("-storepass:file")
        .arg(&key.store_pass);
    if let Some(file) = &key.key_pass {
        sign.arg("-keypass:file").arg(file);
    }
    sign.args(["-sigalg", algorithm, "-digestalg", "SHA-256", "-signedjar"])
        .arg(&signed)
        .arg(&input)
        .arg(&key.alias);
    let ran = capture(&mut sign);
    if !ran.success() {
        return Err(reject(format!(
            "jarsigner could not sign {shown}: {}",
            ran.first_reason()
        )));
    }
    if !signed.is_file() {
        return Err(reject(format!("jarsigner could not sign {shown}")));
    }
    let verified = capture(
        english(&jarsigner)
            .args(["-verify", "-strict", "-verbose", "-certs"])
            .arg(&signed),
    );
    if let Some(error) = verify_error(&verified) {
        return Err(reject(error));
    }
    publish(&signed, &out)?;
    Ok(0)
}
