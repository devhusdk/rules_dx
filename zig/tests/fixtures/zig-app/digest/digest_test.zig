const std = @import("std");
const digest = @import("digest");

test "matches the XXH64 reference vectors" {
    try std.testing.expectEqual(@as(u64, 0xef46db3751d8e999), digest.of(""));
    try std.testing.expectEqual(@as(u64, 0x44bc2cf5ad770999), digest.of("abc"));
}

test "differs for different input" {
    try std.testing.expect(digest.of("lamp") != digest.of("Lamp"));
}
