use std::path::{Path, PathBuf};

use crate::{Ctx, Plugin, link};

const HUB: &str = "@pypi";
const SITE_PACKAGES: &str = "site-packages";
const OUTPUT_GROUP: &str = "install_dir";

pub const PLUGIN: Plugin = Plugin {
    name: "python",
    extensions: &["py"],
    labels: &["python/ide:python_tool"],
    outputs: &["python", SITE_PACKAGES],
    write,
};

fn write(ctx: &Ctx<'_>) -> Result<(), String> {
    let interpreter = ctx
        .outputs
        .iter()
        .find(|path| path.file_name().is_some_and(|name| name != "typescript"))
        .ok_or("the Python interpreter was not built")?;
    link(ctx.dir, "python", interpreter)
        .map_err(|error| format!("linking the Python interpreter: {error}"))?;
    write_site_packages(ctx)
}

/// Links every top-level entry of the locked wheels into `site-packages`, so the editor can import them.
fn write_site_packages(ctx: &Ctx<'_>) -> Result<(), String> {
    let target = ctx.dir.join(SITE_PACKAGES);
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target)
        .map_err(|error| format!("creating {}: {error}", target.display()))?;
    let labels = locked_labels(&ctx.workspace.join("uv.lock"));
    if labels.is_empty() {
        return Ok(());
    }
    let installs = ctx
        .runner
        .build_group(&labels, OUTPUT_GROUP)
        .map_err(|error| format!("building the locked Python packages: {error}"))?;
    for install in installs {
        for site in site_packages_dirs(&install) {
            link_entries(&site, &target)
                .map_err(|error| format!("linking {}: {error}", site.display()))?;
        }
    }
    Ok(())
}

fn locked_labels(lock: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(lock) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut name: Option<String> = None;
    for line in text.lines().chain(["[[package]]"]) {
        if line == "[[package]]" {
            names.extend(name.take());
        } else if let Some(value) = line.strip_prefix("name = \"") {
            name = value.strip_suffix('"').map(normalize);
        } else if line.starts_with("source = ")
            && ["editable", "virtual", "directory"]
                .iter()
                .any(|kind| line.contains(kind))
        {
            name = None;
        }
    }
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| format!("{HUB}//{name}"))
        .collect()
}

fn normalize(name: &str) -> String {
    name.to_lowercase().replace(['-', '.'], "_")
}

fn site_packages_dirs(install: &Path) -> Vec<PathBuf> {
    let Ok(libs) = std::fs::read_dir(install.join("lib")) else {
        return Vec::new();
    };
    libs.flatten()
        .map(|entry| entry.path().join("site-packages"))
        .filter(|path| path.is_dir())
        .collect()
}

fn link_entries(site: &Path, target: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(site).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if target.join(&name).symlink_metadata().is_ok() {
            continue;
        }
        link(target, &name.to_string_lossy(), &entry.path()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_labels_skip_the_workspace_projects() {
        let scratch = dx_test_scratch::scratch("dx-ide-lock-");
        let lock = scratch.path().join("uv.lock");
        std::fs::write(
            &lock,
            "version = 1\n\n[[package]]\nname = \"app\"\nversion = \"0.1\"\nsource = { editable = \".\" }\n\n[[package]]\nname = \"Typing-Extensions\"\nversion = \"4\"\nsource = { registry = \"https://pypi.org/simple\" }\n\n[[package]]\nname = \"requests\"\nversion = \"2\"\nsource = { registry = \"https://pypi.org/simple\" }\n",
        )
        .expect("lock");
        assert_eq!(
            locked_labels(&lock),
            ["@pypi//requests", "@pypi//typing_extensions"]
        );
        scratch.close().expect("cleanup");
    }
}
