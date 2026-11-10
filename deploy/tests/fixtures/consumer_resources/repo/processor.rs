use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [input, output] = &args[args.len() - 2..] else {
        panic!("usage: processor [FLAG]... INPUT OUTPUT");
    };
    let mut bytes = std::fs::read(input).unwrap_or_else(|error| panic!("{input}: {error}"));
    bytes.write_all(b"FOOTER-dx-consumer-v1\n").unwrap();
    std::fs::write(output, bytes).unwrap_or_else(|error| panic!("{output}: {error}"));
}
