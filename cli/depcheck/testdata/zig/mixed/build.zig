const std = @import("std");

pub fn build(b: *std.Build) void {
    const zlib = b.dependency("zlib", .{});
    _ = zlib;
}
