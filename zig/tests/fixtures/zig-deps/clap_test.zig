const std = @import("std");
const clap = @import("clap");

test "clap is importable from the zig_deps hub" {
    const params = comptime clap.parseParamsComptime("-h, --help Display help.\n");
    try std.testing.expectEqual(@as(usize, 1), params.len);
}
