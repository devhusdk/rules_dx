#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

fn report(error: &dx_install_tools::ArgsError) -> i32 {
    if error.0.starts_with("usage:") {
        println!("{error}");
        0
    } else {
        eprintln!("{error}");
        1
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let args = match dx_install_tools::parse_args(argv) {
        Ok(args) => args,
        Err(error) => return report(&error),
    };
    match dx_install_tools::verify(&args, &dx_install_tools::SystemVerifier) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            1
        }
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_of(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn usage_errors_report_success() {
        assert_eq!(
            report(&dx_install_tools::ArgsError(
                "usage: dx_verify --binary PATH".to_owned()
            )),
            0
        );
    }

    #[test]
    fn hard_errors_report_failure() {
        assert_eq!(
            report(&dx_install_tools::ArgsError(
                "dx_verify: missing --binary PATH".to_owned()
            )),
            1
        );
        assert_eq!(
            report(&dx_install_tools::ArgsError(
                "dx_verify: unknown argument '--bogus'".to_owned()
            )),
            1
        );
    }

    #[test]
    fn run_maps_parse_failures_without_touching_the_system() {
        assert_eq!(run(&argv_of(&["dx_verify"])), 1);
        assert_eq!(run(&argv_of(&["dx_verify", "--bogus"])), 1);
        assert_eq!(run(&argv_of(&["dx_verify", "-h"])), 0);
    }
}
