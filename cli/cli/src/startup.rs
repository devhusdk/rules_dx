use std::io;
use std::path::Path;

fn temp_var() -> &'static str {
    if cfg!(windows) {
        "TMP or TEMP"
    } else {
        "TMPDIR"
    }
}

pub fn temp_path_not_utf8() -> String {
    format!(
        "temporary event path is not UTF-8; set {} to a directory with a UTF-8 path",
        temp_var()
    )
}

pub fn temp_dir_failure(base: &Path, error: &io::Error) -> String {
    let var = temp_var();
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
    fn temp_path_not_utf8_names_the_variable() {
        let var = if cfg!(windows) {
            "TMP or TEMP"
        } else {
            "TMPDIR"
        };
        assert_eq!(
            temp_path_not_utf8(),
            format!(
                "temporary event path is not UTF-8; set {var} to a directory with a UTF-8 path"
            )
        );
    }

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
