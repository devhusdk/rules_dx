#[test]
fn nixfmt_is_static() {
    let bytes = std::fs::read(dx_testing::env_file("NIXFMT")).expect("read nixfmt");
    let abis: Vec<dx_binfmt::Abi> = dx_binfmt::inspect(&bytes)
        .expect("inspect nixfmt")
        .into_iter()
        .map(|image| image.abi)
        .collect();
    assert_eq!(abis, [dx_binfmt::Abi::Static]);
}
