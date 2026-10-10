//! Native development entry point over the shared evaluation core.

use shared_core::{eval_expr, CORE_VERSION};

fn main() {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2 * (3 + 4)".to_owned());
    match eval_expr(&input) {
        Ok(value) => println!("ok:{value}"),
        Err(error) => {
            eprintln!("err:{}: {error}", error.kind());
            std::process::exit(1);
        }
    }
    eprintln!("core:{CORE_VERSION}");
}
