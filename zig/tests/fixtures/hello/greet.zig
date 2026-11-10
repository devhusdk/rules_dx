const std = @import("std");

pub fn greeting(buf: []u8, name: []const u8) ![]const u8 {
    return std.fmt.bufPrint(buf, "Hello, {s}!", .{name});
}

test "greeting formats the name" {
    var buf: [32]u8 = undefined;
    try std.testing.expectEqualStrings("Hello, zig!", try greeting(&buf, "zig"));
}
