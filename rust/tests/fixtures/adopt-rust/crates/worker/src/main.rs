use std::env;

fn main() {
    let words: Vec<String> = env::args().skip(1).collect();
    println!("{}", api::digest(words.join(" ")));
}
