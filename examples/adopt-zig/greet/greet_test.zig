const std = @import("std");

test "greet names the caller" {
    const greeting = try @import("greet.zig").greet("zig");
    try std.testing.expect(std.mem.eql(u8, greeting, "hello zig"));
}
