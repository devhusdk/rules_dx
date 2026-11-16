//! The compiler of the nested `own_cc` toolchain: writes `own_cc` to every `-o` output and an empty `-MF` file, also inside `@` parameter files.

use std::fs;

fn expand(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg.strip_prefix('@') {
            Some(params) => {
                let text = fs::read_to_string(params)
                    .unwrap_or_else(|error| panic!("{params}: {error}"));
                out.extend(expand(
                    text.split_whitespace()
                        .map(|word| word.trim_matches(['\'', '"']).to_owned())
                        .collect(),
                ));
            }
            None => out.push(arg),
        }
    }
    out
}

fn main() {
    let args = expand(std::env::args().skip(1).collect());
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let content = match word.as_str() {
            "-o" => "own_cc\n",
            "-MF" => "",
            _ => continue,
        };
        if let Some(path) = words.next() {
            fs::write(path, content).unwrap_or_else(|error| panic!("{path}: {error}"));
        }
    }
}
