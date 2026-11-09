pub fn double(x: u32) u32 {
    const label: []const u8 = x;
    return label;
}

test {
    _ = double(2);
}
