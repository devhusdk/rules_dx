pub fn greet(buf: []u8, name: []const u8) ![]const u8 {
    return std.fmt.bufPrint(buf, "hello {s}", .{name});
}

const std = @import("std");
