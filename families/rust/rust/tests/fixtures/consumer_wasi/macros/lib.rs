use proc_macro::TokenStream;

#[proc_macro]
pub fn expansion_family(_input: TokenStream) -> TokenStream {
    let family = if cfg!(target_family = "wasm") {
        "wasm"
    } else {
        "native"
    };
    format!("{family:?}")
        .parse()
        .expect("a string literal is a token stream")
}
