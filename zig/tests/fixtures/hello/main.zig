const std = @import("std");

pub fn main() !void {
    const msg = try std.fmt.allocPrint(std.heap.page_allocator, "hello {s}\n", .{"zig"});
    _ = msg;
}
