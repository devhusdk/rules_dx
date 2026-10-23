pub fn double(x: u32) u32 {
    return x * 2;
}

test double {
    try @import("std").testing.expectEqual(@as(u32, 4), double(2));
}
