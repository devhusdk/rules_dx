const std = @import("std");

pub fn wrap(buf: []u8, name: []const u8) ![]const u8 {
    return std.fmt.bufPrint(buf, "Hello, {s}!", .{name});
}
