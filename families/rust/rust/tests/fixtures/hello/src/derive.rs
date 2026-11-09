use proc_macro::TokenStream;

#[proc_macro]
pub fn hello_identity(input: TokenStream) -> TokenStream {
    input
}
