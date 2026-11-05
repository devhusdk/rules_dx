use std::path::Path;

use crate::consumer::write;
use crate::macos::tool;

pub const TEAM: &str = "DXTEST0001";

pub fn profile(dir: &Path, name: &str, expires: &str) {
    let key = dir.join("signer.key");
    let cert = dir.join("signer.pem");
    let der = dir.join("signer.der");
    if !cert.exists() {
        tool(
            "openssl",
            &[
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "2",
                "-subj",
                "/CN=dx device test",
                "-keyout",
                &key.to_string_lossy(),
                "-out",
                &cert.to_string_lossy(),
            ],
        );
        tool(
            "openssl",
            &[
                "x509",
                "-in",
                &cert.to_string_lossy(),
                "-outform",
                "DER",
                "-out",
                &der.to_string_lossy(),
            ],
        );
    }
    let encoded = tool("openssl", &["base64", "-A", "-in", &der.to_string_lossy()]);
    let plist = dir.join(format!("{name}.plist"));
    write(
        &plist,
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>AppIDName</key>
  <string>dx device test</string>
  <key>ApplicationIdentifierPrefix</key>
  <array><string>{TEAM}</string></array>
  <key>CreationDate</key>
  <date>2020-01-01T00:00:00Z</date>
  <key>DeveloperCertificates</key>
  <array><data>{encoded}</data></array>
  <key>Entitlements</key>
  <dict>
    <key>application-identifier</key>
    <string>{TEAM}.com.example.device</string>
    <key>com.apple.developer.team-identifier</key>
    <string>{TEAM}</string>
    <key>get-task-allow</key>
    <true/>
  </dict>
  <key>ExpirationDate</key>
  <date>{expires}</date>
  <key>Name</key>
  <string>dx device test</string>
  <key>Platform</key>
  <array><string>iOS</string></array>
  <key>TeamIdentifier</key>
  <array><string>{TEAM}</string></array>
  <key>UUID</key>
  <string>2F6C1D7A-0B3E-4C5D-8E9F-1A2B3C4D5E6F</string>
  <key>Version</key>
  <integer>1</integer>
</dict>
</plist>
"#
        ),
    );
    tool(
        "openssl",
        &[
            "smime",
            "-sign",
            "-nodetach",
            "-binary",
            "-outform",
            "DER",
            "-in",
            &plist.to_string_lossy(),
            "-signer",
            &cert.to_string_lossy(),
            "-inkey",
            &key.to_string_lossy(),
            "-out",
            &dir.join(format!("{name}.mobileprovision"))
                .to_string_lossy(),
        ],
    );
}
