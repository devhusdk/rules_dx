use std::io;
use std::path::Path;

pub fn temp_dir_failure(base: &Path, error: &io::Error) -> String {
    let var = if cfg!(windows) {
        "TMP or TEMP"
    } else {
        "TMPDIR"
    };
    format!(
        "cannot create temporary directory {}: {error}; set {var} to a writable directory",
        base.display()
    )
}

pub fn working_dir_failure(error: &io::Error) -> String {
    format!("cannot read working directory: {error}; cd into an existing directory")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dir_failure_names_directory_and_variable() {
        let error = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let message = temp_dir_failure(Path::new("/var/tmp/dx"), &error);
        let var = if cfg!(windows) {
            "TMP or TEMP"
        } else {
            "TMPDIR"
        };
        assert_eq!(
            message,
            format!(
                "cannot create temporary directory /var/tmp/dx: denied; set {var} to a writable directory"
            )
        );
    }

    #[test]
    fn working_dir_failure_says_to_cd_into_existing_directory() {
        let error = io::Error::new(io::ErrorKind::NotFound, "gone");
        assert_eq!(
            working_dir_failure(&error),
            "cannot read working directory: gone; cd into an existing directory"
        );
    }
}
