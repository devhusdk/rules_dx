const std = @import("std");
const greet = @import("my_project");

test "answer is 42" {
    try std.testing.expectEqual(@as(i32, 42), greet.answer());
}
