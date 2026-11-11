//! Runs the `swift` command of one Xcode.

use crate::util::{Outcome, exit_code, missing};
use crate::xcode::{Tools, XCRUN};

pub const PREFIX: &str = "dx-apple-swift";

/// Runs `xcrun swift <args>` with this process's streams.
pub fn run(tools: &Tools, args: &[String]) -> Outcome {
    let mut argv = vec!["swift"];
    argv.extend(args.iter().map(String::as_str));
    match tools.xcrun_inherit(&argv) {
        None => Err(missing(format!(
            "cannot start {XCRUN}; swift needs a macOS host with Xcode"
        ))),
        code => Ok(exit_code(code)),
    }
}
