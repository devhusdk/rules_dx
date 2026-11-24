use crate::util::{Outcome, exit_code, missing};
use crate::xcode::{Tools, XCRUN};

pub const PREFIX: &str = "dx-apple-swift";

/// # Errors
/// Returns an error when the operation fails.
pub fn run(tools: &Tools<'_>, tool: &str, args: &[String]) -> Outcome {
    let mut argv = vec![tool];
    argv.extend(args.iter().map(String::as_str));
    match tools.xcrun_inherit(&argv) {
        None => Err(missing(format!(
            "cannot start {XCRUN}; {tool} needs a macOS host with Xcode"
        ))),
        code => Ok(exit_code(code)),
    }
}
