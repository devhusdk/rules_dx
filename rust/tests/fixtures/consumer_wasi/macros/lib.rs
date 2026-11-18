use proc_macro::{Literal, TokenStream, TokenTree};

#[proc_macro]
pub fn expansion_family(_input: TokenStream) -> TokenStream {
    let family = if cfg!(target_family = "wasm") {
        "wasm"
    } else {
        "native"
    };
    TokenStream::from(TokenTree::Literal(Literal::string(family)))
}
