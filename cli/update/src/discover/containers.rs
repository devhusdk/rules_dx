//! Lists `oci.pull` base images whose digest lags the registry's `latest` tag.

use super::{Candidate, Fetch, Findings, LookupError};
use dx_module_bazel::ModuleFile;

pub const SET: &str = "containers";

/// Checks each `oci.pull` of the module against the digest its image's `latest` tag has now.
pub fn outdated(module: &ModuleFile, only: &[String], fetch: &dyn Fetch) -> Findings {
    let mut found = Findings::default();
    for call in module.calls_named("oci.pull") {
        let (Some(name), Some(image), Some(current)) =
            (call.str("name"), call.str("image"), call.str("digest"))
        else {
            continue;
        };
        if !super::wanted(only, name) {
            continue;
        }
        let Some(url) = manifest_url(image) else {
            found.errors.push(problem(
                name,
                format!("image `{image}` has no registry host"),
            ));
            continue;
        };
        match fetch.digest(&url) {
            Ok(latest) if latest != current => found.candidates.push(Candidate {
                set: SET.to_owned(),
                package: name.to_owned(),
                current: current.to_owned(),
                latest,
                published: None,
                prerelease: false,
                repo: None,
                tags: None,
            }),
            Ok(_) => {}
            Err(reason) => found.errors.push(problem(name, reason)),
        }
    }
    found
}

/// The registry URL of the `latest` manifest of `host/path`.
fn manifest_url(image: &str) -> Option<String> {
    let (host, path) = image.split_once('/')?;
    (host.contains('.') && !path.is_empty())
        .then(|| format!("https://{host}/v2/{path}/manifests/latest"))
}

fn problem(package: &str, reason: String) -> LookupError {
    LookupError {
        set: SET.to_owned(),
        package: package.to_owned(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const NEW: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";

    struct Digests;

    impl Fetch for Digests {
        fn get(&self, _url: &str) -> Result<String, String> {
            Err("unused".to_owned())
        }

        fn digest(&self, url: &str) -> Result<String, String> {
            match url {
                "https://gcr.io/v2/distroless/static-debian12/manifests/latest" => {
                    Ok(NEW.to_owned())
                }
                "https://gcr.io/v2/distroless/cc-debian12/manifests/latest" => Ok(OLD.to_owned()),
                _ => Err("HTTP 404".to_owned()),
            }
        }
    }

    fn module() -> ModuleFile {
        let pull = |name: &str, image: &str| {
            format!("oci.pull(name = \"{name}\", image = \"{image}\", digest = \"{OLD}\")\n")
        };
        let text = pull("distroless_static", "gcr.io/distroless/static-debian12")
            + &pull("distroless_cc", "gcr.io/distroless/cc-debian12")
            + &pull("gone", "gcr.io/distroless/gone");
        ModuleFile::parse(&text).expect("parse")
    }

    #[test]
    fn reports_a_moved_tag_and_a_failed_lookup() {
        let found = outdated(&module(), &[], &Digests);
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].package, "distroless_static");
        assert_eq!(found.candidates[0].current, OLD);
        assert_eq!(found.candidates[0].latest, NEW);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "gone");
    }

    #[test]
    fn narrows_to_the_named_pull() {
        let only = ["distroless_cc".to_owned()];
        assert_eq!(outdated(&module(), &only, &Digests), Findings::default());
    }

    #[test]
    fn an_image_without_a_host_is_an_error() {
        let module = ModuleFile::parse(&format!(
            "oci.pull(name = \"x\", image = \"ubuntu\", digest = \"{OLD}\")\n"
        ))
        .expect("parse");
        assert_eq!(outdated(&module, &[], &Digests).errors.len(), 1);
    }
}
