//! Signs a device app into a distributable IPA.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::signing::{
    Flags, Keychain, absolute, base64_decode, caller, codesign, detail, entries, fingerprint, keys,
    output_path, private_files, read_identity, text, value,
};
use crate::util::{Env, Outcome, Scratch, missing, reject, usage};
use crate::xcode::{CODESIGN, DITTO, PLUTIL, SECURITY, Tools};

pub const PREFIX: &str = "dx-apple-export";

const HELP: &str = "usage: bazel run <export> -- --certificate <identity.p12> --password-file <file> --out <App.ipa>";

/// The sorted file names of an IPA, without directories and signatures.
pub fn listing(ipa: &Path) -> Option<BTreeSet<String>> {
    let archive = zip::ZipArchive::new(std::fs::File::open(ipa).ok()?).ok()?;
    Some(
        archive
            .file_names()
            .filter_map(Result::ok)
            .filter(|name| !name.ends_with('/') && !name.contains("/_CodeSignature/"))
            .map(|name| name.into_owned())
            .collect(),
    )
}

fn digits(stamp: &str) -> String {
    stamp
        .chars()
        .filter(char::is_ascii_digit)
        .take(14)
        .collect()
}

/// The current UTC time as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The platform and minimum OS of the first `LC_BUILD_VERSION` in `otool -l` output.
pub fn build_version(listing: &str) -> (String, String) {
    let (mut command, mut platform, mut minos) = (String::new(), String::new(), String::new());
    for line in listing.lines() {
        let mut fields = line.split_whitespace();
        let (Some(first), Some(second)) = (fields.next(), fields.next()) else {
            continue;
        };
        if first == "cmd" {
            command = second.to_owned();
        } else if command == "LC_BUILD_VERSION" && first == "platform" && platform.is_empty() {
            platform = second.to_owned();
        } else if command == "LC_BUILD_VERSION" && first == "minos" && minos.is_empty() {
            minos = second.to_owned();
        }
    }
    (platform, minos)
}

fn covers(app_identifier: &str, team: &str, bundle_id: &str) -> bool {
    let wanted = format!("{team}.{bundle_id}");
    match app_identifier.strip_suffix('*') {
        Some(prefix) => wanted.starts_with(prefix),
        None => app_identifier == wanted,
    }
}

/// Exports the IPA `args[1]` signed with the profile `args[2]` as bundle id `args[0]`; the flags follow.
pub fn run(tools: &Tools, args: &[String], env: Env) -> Outcome {
    private_files();
    let [bundle_id, ipa, profile, flag_args @ ..] = args else {
        return Err(usage("run the export launcher with bazel run"));
    };
    let flags = Flags::parse(flag_args, &["certificate", "password-file", "out"], HELP)?;
    for (name, what) in [
        ("certificate", "names the signing identity .p12 file"),
        ("password-file", "names the file holding the .p12 password"),
        ("out", "names the .ipa to write"),
    ] {
        if flags.get(name).is_empty() {
            return Err(usage(format!("--{name} {what}; {HELP}")));
        }
    }
    let caller = caller(env);
    let cert = absolute(flags.get("certificate"), &caller);
    let password_file = absolute(flags.get("password-file"), &caller);
    let out = absolute(flags.get("out"), &caller);
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
    let out = output_path(&out, env, ".ipa", "an .ipa file")?;
    if bundle_id.is_empty() {
        return Err(usage("the launcher names no bundle_id"));
    }
    let (ipa, profile) = (PathBuf::from(ipa), PathBuf::from(profile));
    if !ipa.is_file() {
        return Err(usage(format!("built IPA {} is missing", ipa.display())));
    }
    if !profile.is_file() {
        return Err(usage(format!(
            "declared profile {} is missing",
            profile.display()
        )));
    }

    let work = Scratch::new(env, "export")?;
    let work_dir = work.path();
    let unpacked = work_dir.join("unpacked");
    let unzipped = std::fs::File::open(&ipa)
        .ok()
        .and_then(|file| zip::ZipArchive::new(file).ok())
        .and_then(|mut archive| archive.extract(&unpacked).ok());
    if unzipped.is_none() {
        return Err(usage(format!("cannot unpack {}", ipa.display())));
    }
    let bundles: Vec<PathBuf> = entries(&unpacked.join("Payload"))
        .into_iter()
        .filter(|path| path.extension().is_some_and(|end| end == "app") && path.is_dir())
        .collect();
    let app = match bundles.as_slice() {
        [] => {
            return Err(usage(format!(
                "{} holds no Payload/*.app bundle",
                ipa.display()
            )));
        }
        [one] => one.clone(),
        _ => {
            return Err(usage(format!(
                "{} holds more than one Payload/*.app bundle",
                ipa.display()
            )));
        }
    };
    if app.join("PlugIns").exists() {
        return Err(usage(
            "the IPA carries app extensions; export signs plain apps only",
        ));
    }
    let name = app
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let before = listing(&ipa).unwrap_or_default();

    let embedded = std::fs::read(app.join("embedded.mobileprovision"))
        .map_err(|_| usage(format!("{} embeds no provisioning profile", ipa.display())))?;
    if std::fs::read(&profile).ok().as_deref() != Some(embedded.as_slice()) {
        return Err(usage(format!(
            "{} embeds a provisioning profile other than the declared {}",
            ipa.display(),
            profile.display()
        )));
    }

    let decoded_text = tools.tool(SECURITY, &["cms", "-D", "-i", &text(&profile)]);
    if !decoded_text.success() {
        return Err(usage(format!(
            "cannot decode the provisioning profile {}",
            profile.display()
        )));
    }
    let decoded = work_dir.join("profile.plist");
    std::fs::write(&decoded, &decoded_text.stdout).map_err(|e| usage(e.to_string()))?;
    let shown = profile.display();
    let team = value(tools, "TeamIdentifier.0", &decoded).ok_or_else(|| {
        usage(format!(
            "provisioning profile {shown} names no TeamIdentifier"
        ))
    })?;
    let profile_name = value(tools, "Name", &decoded).unwrap_or_default();
    let profile_uuid = value(tools, "UUID", &decoded).unwrap_or_default();
    let expires = value(tools, "ExpirationDate", &decoded).ok_or_else(|| {
        usage(format!(
            "provisioning profile {shown} names no ExpirationDate"
        ))
    })?;
    let current = env("DX_NOW").unwrap_or_else(now);
    let (expires_digits, current_digits) = (digits(&expires), digits(&current));
    if expires_digits.len() != 14 || current_digits.len() != 14 {
        return Err(usage(format!(
            "cannot read the profile ExpirationDate '{expires}'"
        )));
    }
    if expires_digits <= current_digits {
        return Err(usage(format!(
            "provisioning profile {shown} expired on {expires}"
        )));
    }
    let app_identifier =
        value(tools, "Entitlements.application-identifier", &decoded).ok_or_else(|| {
            usage(format!(
                "provisioning profile {shown} names no application-identifier"
            ))
        })?;
    if !covers(&app_identifier, &team, bundle_id) {
        return Err(usage(format!(
            "provisioning profile {shown} covers {app_identifier}, not {team}.{bundle_id}"
        )));
    }

    let allowed = tools.tool(
        PLUTIL,
        &[
            "-extract",
            "Entitlements",
            "xml1",
            "-o",
            "-",
            &text(&decoded),
        ],
    );
    if !allowed.success() {
        return Err(usage(format!(
            "provisioning profile {shown} names no Entitlements"
        )));
    }
    let allowed_keys = keys(&allowed.stdout);
    let signed = tools.tool(
        CODESIGN,
        &["-d", "--entitlements", "-", "--xml", &text(&app)],
    );
    let unreadable = || usage(format!("cannot read the entitlements of {name}"));
    if !signed.success() {
        return Err(unreadable());
    }
    let mut entitlements: Option<PathBuf> = None;
    let mut app_keys = BTreeSet::new();
    if !signed.stdout.is_empty() {
        let file = work_dir.join("entitlements.plist");
        std::fs::write(&file, &signed.stdout).map_err(|_| unreadable())?;
        if !tools
            .tool(PLUTIL, &["-convert", "xml1", &text(&file)])
            .success()
        {
            return Err(unreadable());
        }
        app_keys = keys(&std::fs::read_to_string(&file).unwrap_or_default());
        entitlements = Some(file);
    }
    if let Some(extra) = app_keys.difference(&allowed_keys).next() {
        return Err(usage(format!(
            "entitlement '{extra}' is not allowed by the provisioning profile {shown}"
        )));
    }

    let identity = read_identity(tools, work_dir, &cert, &password_file)?;
    let sha1 = identity.sha1.clone();
    let mut listed = false;
    for index in 0.. {
        let Some(encoded) = value(tools, &format!("DeveloperCertificates.{index}"), &decoded)
        else {
            break;
        };
        let Some(bytes) = base64_decode(&encoded) else {
            continue;
        };
        let file = work_dir.join("profile-cert.der");
        if std::fs::write(&file, bytes).is_ok()
            && fingerprint(tools, &file, "DER").as_deref() == Some(sha1.as_str())
        {
            listed = true;
            break;
        }
    }
    if !listed {
        return Err(usage(format!(
            "certificate {sha1} is not in the provisioning profile {shown}"
        )));
    }

    let keychain = Keychain::open(tools, work_dir, &identity, &cert, "export")?;
    let keychain_text = text(&keychain.path);
    let frameworks = app.join("Frameworks");
    if frameworks.is_dir() {
        for nested in entries(&frameworks) {
            codesign(
                tools,
                &[
                    "--force",
                    "--keychain",
                    &keychain_text,
                    "--sign",
                    &sha1,
                    &text(&nested),
                ],
            )?;
        }
    }
    let entitlements_text = entitlements.as_ref().map(|path| text(path));
    let mut sign = vec![
        "--force",
        "--keychain",
        keychain_text.as_str(),
        "--sign",
        sha1.as_str(),
    ];
    if let Some(path) = &entitlements_text {
        sign.extend(["--entitlements", path]);
    }
    let app_text = text(&app);
    sign.push(&app_text);
    codesign(tools, &sign)?;

    let signed_ipa = work_dir.join("signed.ipa");
    let payload = text(&unpacked.join("Payload"));
    let zipped = tools.tool(
        DITTO,
        &[
            "-c",
            "-k",
            "--sequesterRsrc",
            "--keepParent",
            &payload,
            &text(&signed_ipa),
        ],
    );
    if !zipped.success() {
        return Err(usage("cannot write the signed IPA"));
    }

    if !tools
        .tool(CODESIGN, &["--verify", "--deep", "--strict", &app_text])
        .success()
    {
        return Err(reject(format!(
            "signed {name} fails codesign --verify --deep --strict"
        )));
    }
    let details = crate::signing::describe(tools, &["-dvv"], &app)
        .ok_or_else(|| reject(format!("cannot read the signature of {name}")))?;
    let signed_team = detail(&details, "TeamIdentifier=");
    let authority = detail(&details, "Authority=");
    if signed_team != team {
        return Err(reject(format!(
            "signed {name} carries TeamIdentifier '{signed_team}', want {team}"
        )));
    }
    if authority.is_empty() {
        return Err(reject(format!(
            "signed {name} carries no signing authority"
        )));
    }
    let executable = value(tools, "CFBundleExecutable", &app.join("Info.plist"))
        .ok_or_else(|| reject(format!("{name} names no CFBundleExecutable")))?;
    let binary = text(&app.join(&executable));
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
    if archs != "arm64" {
        return Err(reject(format!("{executable} carries {archs}, want arm64")));
    }
    let otool = tools.xcrun(&["otool", "-l", &binary]);
    let (platform, minos) = build_version(&otool.stdout);
    if platform != "2" {
        return Err(reject(format!(
            "{executable} is built for platform '{platform}', want 2 (iphoneos)"
        )));
    }
    let after = listing(&signed_ipa).unwrap_or_default();
    if let Some(lost) = before.symmetric_difference(&after).next() {
        return Err(reject(format!(
            "signing changed the bundle contents at {lost}"
        )));
    }

    let entitlement_keys = app_keys.into_iter().collect::<Vec<_>>().join(",");
    std::fs::copy(&signed_ipa, &out)
        .map_err(|e| usage(format!("cannot write {}: {e}", out.display())))?;
    let notes = PathBuf::from(format!("{}.txt", out.display()));
    let report = format!(
        "bundle_id={bundle_id}\nteam={team}\nprofile_name={profile_name}\nprofile_uuid={profile_uuid}\nprofile_expires={expires}\ncertificate_sha1={sha1}\nauthority={authority}\nentitlements={entitlement_keys}\narchs={archs}\nplatform=iphoneos\nminos={minos}\n"
    );
    std::fs::write(&notes, report)
        .map_err(|e| usage(format!("cannot write {}: {e}", notes.display())))?;
    println!("{PREFIX}: wrote {} and {}", out.display(), notes.display());
    Ok(0)
}
