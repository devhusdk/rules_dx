const std = @import("std");

pub fn greet(name: []const u8) ![]u8 {
    return std.fmt.allocPrint(std.heap.page_allocator, "hello {s}", .{name});
}
