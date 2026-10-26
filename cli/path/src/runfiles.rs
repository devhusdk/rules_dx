//! Finding the runfiles that belong to one binary.

use std::io;
use std::path::{Path, PathBuf};

use runfiles::{Runfiles, RunfilesBuilder};

/// Names the runfiles manifest that sits beside one binary.
///
/// A launcher keeps its own suffix on some hosts and drops it on others, so try
/// the name as written and the name without its suffix.
pub fn manifest_beside(binary: &Path) -> Option<PathBuf> {
    let bare = binary.with_extension("");
    [binary.to_path_buf(), bare]
        .into_iter()
        .map(|name| PathBuf::from(format!("{}.runfiles_manifest", dx_path_display(&name))))
        .find(|candidate| candidate.is_file())
}

/// Names the runfiles manifest for one binary, preferring the one on disk.
pub fn manifest_for(binary: &Path) -> PathBuf {
    let named = format!("{}.runfiles_manifest", dx_path_display(binary));
    manifest_beside(binary).unwrap_or_else(|| PathBuf::from(named))
}

/// The runfiles of one binary, read with the upstream runfiles library.
///
/// Only the files beside the binary are read, never `RUNFILES_MANIFEST_FILE`,
/// which describes whatever started this process.
#[derive(Debug)]
pub struct Resolver {
    inner: Runfiles,
    source: PathBuf,
}

impl Resolver {
    /// Reads the manifest beside one binary, then a populated tree, then the tree's MANIFEST.
    pub fn for_binary(binary: &Path) -> io::Result<Self> {
        if let Some(manifest) = manifest_beside(binary) {
            let text = std::fs::read_to_string(&manifest)?;
            return Resolver::new(Runfiles::builder().manifest(text), manifest);
        }
        if let Some(tree) = tree_beside(binary) {
            let manifest = tree.join("MANIFEST");
            if !populated(&tree) && manifest.is_file() {
                let text = std::fs::read_to_string(&manifest)?;
                return Resolver::new(Runfiles::builder().manifest(text), manifest);
            }
            return Resolver::new(Runfiles::builder().directory(&tree), tree);
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no runfiles beside {}: read {}",
                binary.display(),
                manifest_for(binary).display()
            ),
        ))
    }

    /// Names the manifest or tree this resolver read.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Reads the runfiles tree rooted at one directory.
    pub fn for_tree(tree: &Path) -> io::Result<Self> {
        if !tree.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no runfiles tree at {}", tree.display()),
            ));
        }
        Resolver::new(Runfiles::builder().directory(tree), tree.to_path_buf())
    }

    /// Returns the file one key names, read as this runfiles' own repository.
    pub fn lookup(&self, key: &str) -> io::Result<PathBuf> {
        self.lookup_from(key, "")
    }

    /// Returns the file one key names, read as `source_repo` spells it.
    pub fn lookup_from(&self, key: &str, source_repo: &str) -> io::Result<PathBuf> {
        self.inner.rlocation_from(key, source_repo).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{key} is not in {}", self.source.display()),
            )
        })
    }

    fn new(builder: RunfilesBuilder, source: PathBuf) -> io::Result<Self> {
        let inner = builder
            .build()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        Ok(Resolver { inner, source })
    }
}

fn tree_beside(binary: &Path) -> Option<PathBuf> {
    let dir = binary.parent()?;
    let bare = binary.with_extension("");
    [binary.to_path_buf(), bare]
        .into_iter()
        .map(|name| dir.join(format!("{}.runfiles", dx_path_display(&name))))
        .find(|candidate| candidate.is_dir())
}

fn populated(tree: &Path) -> bool {
    std::fs::read_dir(tree).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some("MANIFEST" | "_repo_mapping")
            )
        })
    })
}

fn dx_path_display(path: &Path) -> String {
    super::spell(path, super::Spelling::Native)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_resolves_from_an_explicit_tree_root() {
        let dir = scratch("explicit-tree");
        let target = write(&dir, "tree/_main/pkg/tool.txt", "from the tree\n");
        let resolver = Resolver::for_tree(&dir.join("tree")).expect("resolver");
        assert_eq!(found(&resolver, "_main/pkg/tool.txt"), target);
    }

    #[test]
    fn an_explicit_tree_root_must_exist() {
        let dir = scratch("missing-tree");
        let error = Resolver::for_tree(&dir.join("absent")).expect_err("no such tree");
        assert!(error.to_string().contains("no runfiles tree"), "{error}");
    }

    #[test]
    fn a_missing_manifest_names_the_one_the_binary_asked_for() {
        let missing = manifest_for(Path::new("/out/bin/prettier.exe"));
        assert!(missing.ends_with("/out/bin/prettier.exe.runfiles_manifest"));
    }

    #[test]
    fn a_suffixless_binary_keeps_its_name() {
        let missing = manifest_for(Path::new("/out/bin/prettier"));
        assert!(missing.ends_with("/out/bin/prettier.runfiles_manifest"));
    }

    #[test]
    fn a_key_resolves_from_the_manifest_beside_the_binary() {
        let dir = scratch("manifest");
        let tool = write(&dir, "tool.txt", "from the manifest\n");
        let binary = write(&dir, "tool.exe", "not really a binary\n");
        let manifest = write(
            &dir,
            "tool.exe.runfiles_manifest",
            &format!("_main/pkg/tool.txt {}\n", tool.display()),
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), manifest);
        assert_eq!(found(&resolver, "_main/pkg/tool.txt"), tool);
    }

    fn escaped(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "\\b")
            .replace('\n', "\\n")
    }

    #[test]
    fn a_key_with_spaces_and_escapes_resolves() {
        let dir = scratch("escapes");
        let binary = write(&dir, "tool.exe", "binary\n");
        let spaced = write(&dir, "a b/c.txt", "spaced\n");
        let tabbed = write(&dir, "tabbed/c.txt", "tabbed\n");
        let manifest = write(
            &dir,
            "tool.exe.runfiles_manifest",
            &format!(
                " _main/a\\sb/c\t {}\n _main/a\tb/c\\n {}\n_main/plain out/plain\n",
                escaped(&spaced),
                escaped(&tabbed)
            ),
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), manifest);
        assert_eq!(found(&resolver, "_main/a b/c\t"), spaced);
        assert_eq!(found(&resolver, "_main/a\tb/c\n"), tabbed);
        assert_eq!(found(&resolver, "_main/plain"), PathBuf::from("out/plain"));
    }

    #[test]
    fn a_suffixless_manifest_beside_a_suffixed_binary_is_read() {
        let dir = scratch("suffixless");
        let binary = write(&dir, "tool.exe", "binary\n");
        let manifest = write(
            &dir,
            "tool.runfiles_manifest",
            "_main/pkg/tool.txt out/tool\n",
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), manifest);
        assert_eq!(
            found(&resolver, "_main/pkg/tool.txt"),
            PathBuf::from("out/tool")
        );
    }

    #[test]
    fn a_key_from_another_repository_resolves_through_the_repo_mapping() {
        let dir = scratch("mapping");
        let binary = write(&dir, "tool.exe", "binary\n");
        let mapping = write(&dir, "repo_mapping", ",dep,dep+1.0.0+\n");
        let manifest = write(
            &dir,
            "tool.exe.runfiles_manifest",
            &format!(
                "_repo_mapping {}\ndep+1.0.0+/pkg/tool.txt out/tool.txt\n",
                mapping.display()
            ),
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), manifest);
        assert_eq!(
            found(&resolver, "dep/pkg/tool.txt"),
            PathBuf::from("out/tool.txt")
        );
        assert!(resolver
            .lookup_from("dep/pkg/tool.txt", "other_repo")
            .is_err());
    }

    #[test]
    fn an_absolute_key_is_returned_as_written() {
        let dir = scratch("absolute");
        let binary = write(&dir, "tool.exe", "binary\n");
        write(
            &dir,
            "tool.exe.runfiles_manifest",
            "_main/pkg/tool.txt out/tool\n",
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        let absolute = dir.join("elsewhere");
        let key = absolute.to_string_lossy().into_owned();
        assert_eq!(found(&resolver, &key), absolute);
    }

    #[test]
    fn a_key_resolves_from_the_tree_beside_the_binary() {
        let dir = scratch("tree");
        let binary = write(&dir, "tool.exe", "binary\n");
        let target = write(
            &dir,
            "tool.exe.runfiles/_main/pkg/tool.txt",
            "from the tree\n",
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), dir.join("tool.exe.runfiles"));
        assert_eq!(found(&resolver, "_main/pkg/tool.txt"), target);
    }

    #[test]
    fn the_manifest_inside_the_tree_beside_the_binary_is_read() {
        let dir = scratch("tree-manifest");
        let binary = write(&dir, "tool.exe", "binary\n");
        let mapping = write(&dir, "repo_mapping", ",dep,dep+\n");
        let tool = write(&dir, "real/tool.txt", "from the tree manifest\n");
        write(
            &dir,
            "tool.exe.runfiles/MANIFEST",
            &format!(
                "_repo_mapping {}\ndep+/pkg/tool.txt {}\n",
                mapping.display(),
                tool.display()
            ),
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), dir.join("tool.exe.runfiles/MANIFEST"));
        assert_eq!(found(&resolver, "dep/pkg/tool.txt"), tool);
        assert_eq!(found(&resolver, "dep+/pkg/tool.txt"), tool);
        assert!(resolver.lookup("dep/pkg/absent.txt").is_err());
    }

    #[test]
    fn a_populated_tree_wins_over_its_stale_manifest() {
        let dir = scratch("stale-tree-manifest");
        let binary = write(&dir, "tool.exe", "binary\n");
        let tool = write(
            &dir,
            "tool.exe.runfiles/_main/pkg/tool.txt",
            "from the tree\n",
        );
        write(&dir, "tool.exe.runfiles/_repo_mapping", ",dep,dep+\n");
        let data = write(
            &dir,
            "tool.exe.runfiles/dep+/pkg/data.txt",
            "from the dep\n",
        );
        write(
            &dir,
            "tool.exe.runfiles/MANIFEST",
            "_repo_mapping /build-machine/out/_repo_mapping\n\
             _main/pkg/tool.txt /build-machine/out/tool.txt\n\
             dep+/pkg/data.txt /build-machine/out/data.txt\n",
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), dir.join("tool.exe.runfiles"));
        assert_eq!(found(&resolver, "_main/pkg/tool.txt"), tool);
        assert_eq!(found(&resolver, "dep/pkg/data.txt"), data);
    }

    #[test]
    fn a_tree_with_only_a_manifest_and_mapping_reads_the_manifest() {
        let dir = scratch("tree-manifest-mapping");
        let binary = write(&dir, "tool.exe", "binary\n");
        let mapping = write(&dir, "real/repo_mapping", ",dep,dep+\n");
        let tool = write(&dir, "real/tool.txt", "from the tree manifest\n");
        write(&dir, "tool.exe.runfiles/_repo_mapping", ",dep,dep+\n");
        let manifest = write(
            &dir,
            "tool.exe.runfiles/MANIFEST",
            &format!(
                "_repo_mapping {}\ndep+/pkg/tool.txt {}\n",
                mapping.display(),
                tool.display()
            ),
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), manifest);
        assert_eq!(found(&resolver, "dep/pkg/tool.txt"), tool);
        assert!(resolver.lookup("dep/pkg/absent.txt").is_err());
    }

    #[test]
    fn a_suffixless_tree_beside_a_suffixed_binary_is_read() {
        let dir = scratch("suffixless-tree");
        let binary = write(&dir, "tool.exe", "binary\n");
        write(&dir, "tool.runfiles/_main/pkg/tool.txt", "from the tree\n");
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(
            found(&resolver, "_main/pkg/tool.txt"),
            dir.join("tool.runfiles/_main/pkg/tool.txt")
        );
    }

    #[test]
    fn a_manifest_wins_over_the_tree_beside_the_binary() {
        let dir = scratch("both");
        let binary = write(&dir, "tool.exe", "binary\n");
        write(
            &dir,
            "tool.exe.runfiles/_main/pkg/tool.txt",
            "from the tree\n",
        );
        let manifest = write(
            &dir,
            "tool.exe.runfiles_manifest",
            "_main/pkg/tool.txt out/from-the-manifest.txt\n",
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        assert_eq!(resolver.source(), manifest);
        assert_eq!(
            found(&resolver, "_main/pkg/tool.txt"),
            PathBuf::from("out/from-the-manifest.txt")
        );
    }

    #[test]
    fn a_missing_key_reports_the_key_and_the_file_it_read() {
        let dir = scratch("missing-key");
        let binary = write(&dir, "tool.exe", "binary\n");
        let manifest = write(
            &dir,
            "tool.exe.runfiles_manifest",
            "_main/pkg/tool.txt out/tool.txt\n",
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        let error = resolver
            .lookup("_main/pkg/absent.txt")
            .expect_err("no such key");
        let message = error.to_string();
        assert!(
            message.contains("_main/pkg/absent.txt"),
            "key missing from: {message}"
        );
        assert!(
            message.contains(&manifest.display().to_string()),
            "file missing from: {message}"
        );
    }

    #[test]
    fn a_binary_with_no_runfiles_reports_the_file_it_looked_for() {
        let dir = scratch("no-runfiles");
        let binary = write(&dir, "tool.exe", "binary\n");
        let error = Resolver::for_binary(&binary).expect_err("nothing beside the binary");
        let message = error.to_string();
        assert!(
            message.contains(&binary.display().to_string()),
            "binary missing from: {message}"
        );
        assert!(
            message.contains("runfiles_manifest"),
            "manifest name missing from: {message}"
        );
    }

    #[test]
    fn a_malformed_repository_mapping_is_reported() {
        let dir = scratch("bad-mapping");
        let binary = write(&dir, "tool.exe", "binary\n");
        let mapping = write(&dir, "repo_mapping", "not,enough\n");
        write(
            &dir,
            "tool.exe.runfiles_manifest",
            &format!("_repo_mapping {}\n", mapping.display()),
        );
        let error = Resolver::for_binary(&binary).expect_err("bad mapping");
        assert!(
            error.to_string().contains("RepoMappingInvalidFormat"),
            "error text: {error}"
        );
    }

    fn found(resolver: &Resolver, key: &str) -> PathBuf {
        resolver.lookup(key).expect("runfile")
    }

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("dx-path-runfiles-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(&path, body).expect("write");
        path
    }
}
