const std = @import("std");
const util = @import("util.zig");

pub fn greeting(buf: []u8, name: []const u8) ![]const u8 {
    return util.wrap(buf, name);
}
