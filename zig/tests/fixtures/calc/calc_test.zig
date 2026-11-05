const std = @import("std");
const calc = @import("calc");

test "clamp raises values below the range" {
    try std.testing.expectEqual(@as(i32, 0), calc.clamp(-5, 0, 10));
}

test "clamp keeps values inside the range" {
    try std.testing.expectEqual(@as(i32, 4), calc.clamp(4, 0, 10));
}
