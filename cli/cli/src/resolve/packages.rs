use std::path::Path;

use super::ResolveError;

#[derive(Default)]
pub(crate) struct PackageCache {
    is_package: std::collections::HashMap<String, bool>,
}

impl PackageCache {
    fn is_package(&mut self, workspace: &Path, dir: &str) -> bool {
        if let Some(hit) = self.is_package.get(dir) {
            return *hit;
        }
        let base = if dir.is_empty() {
            workspace.to_path_buf()
        } else {
            workspace.join(dir)
        };
        let found = dx_path::bazel_package::is_package_dir(&base);
        self.is_package.insert(dir.to_owned(), found);
        found
    }

    fn enclosing(&mut self, workspace: &Path, rel: &str) -> Option<(String, String)> {
        let mut dir = match rel.rfind('/') {
            Some(index) => &rel[..index],
            None => "",
        };
        loop {
            if self.is_package(workspace, dir) {
                let in_package = if dir.is_empty() {
                    rel.to_owned()
                } else {
                    rel[dir.len() + 1..].to_owned()
                };
                return Some((dir.to_owned(), in_package));
            }
            if dir.is_empty() {
                return None;
            }
            dir = match dir.rfind('/') {
                Some(index) => &dir[..index],
                None => "",
            };
        }
    }

    pub(crate) fn file_label(
        &mut self,
        workspace: &Path,
        rel: &str,
        scope: &str,
    ) -> Result<String, ResolveError> {
        match self.enclosing(workspace, rel) {
            Some((package, in_package)) => {
                if package.is_empty() {
                    Ok(format!("//:{in_package}"))
                } else {
                    Ok(format!("//{package}:{in_package}"))
                }
            }
            None => Err(ResolveError::NotAPackage {
                scope: scope.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(workspace: &Path, rel: &str, text: &str) {
        let full = workspace.join(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("parent dir");
        std::fs::write(full, text).expect("write file");
    }

    #[test]
    fn file_label_uses_nearest_enclosing_package() {
        let scratch = dx_test_scratch::scratch("dx-resolve-packages-nearest-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/sub/BUILD.bazel", "");
        let mut cache = PackageCache::default();
        assert_eq!(
            cache
                .file_label(&workspace, "pkg/sub/a.py", "pkg/sub/a.py")
                .expect("label"),
            "//pkg/sub:a.py",
        );
        assert_eq!(
            cache
                .file_label(&workspace, "pkg/a.py", "pkg/a.py")
                .expect("label"),
            "//pkg:a.py",
        );
    }

    #[test]
    fn file_label_maps_root_and_reports_missing_package() {
        let scratch = dx_test_scratch::scratch("dx-resolve-packages-root-missing-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD.bazel", "");
        write(&workspace, "top.py", "x = 1\n");
        let mut cache = PackageCache::default();
        assert_eq!(
            cache
                .file_label(&workspace, "top.py", "top.py")
                .expect("label"),
            "//:top.py",
        );
        let scratch = dx_test_scratch::scratch("dx-resolve-packages-missing-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/a.py", "x = 1\n");
        let mut cache = PackageCache::default();
        assert!(matches!(
            cache.file_label(&workspace, "pkg/a.py", "pkg/a.py"),
            Err(ResolveError::NotAPackage { .. })
        ));
    }

    #[test]
    fn file_label_skips_build_directories() {
        let scratch = dx_test_scratch::scratch("dx-resolve-packages-build-dir-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD.bazel", "");
        write(&workspace, "web/src/app.ts", "");
        write(&workspace, "web/BUILD/bundle.js", "");
        let mut cache = PackageCache::default();
        assert_eq!(
            cache
                .file_label(&workspace, "web/src/app.ts", "web/src/app.ts")
                .expect("label"),
            "//:web/src/app.ts",
        );
        let scratch = dx_test_scratch::scratch("dx-resolve-packages-build-dir-only-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD/out.js", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let mut cache = PackageCache::default();
        assert!(matches!(
            cache.file_label(&workspace, "pkg/a.py", "pkg/a.py"),
            Err(ResolveError::NotAPackage { .. })
        ));
    }

    #[test]
    fn file_label_ignores_build_contents() {
        let scratch = dx_test_scratch::scratch("dx-resolve-packages-contents-");
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "pkg/BUILD.bazel",
            "this is not valid starlark ((((",
        );
        write(&workspace, "pkg/a.py", "x = 1\n");
        let mut cache = PackageCache::default();
        assert_eq!(
            cache
                .file_label(&workspace, "pkg/a.py", "pkg/a.py")
                .expect("label"),
            "//pkg:a.py",
        );
    }
}
