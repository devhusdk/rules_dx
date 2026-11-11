const std = @import("std");
const conformance = @import("conformance");

test "passing_case" {
    var buf: [32]u8 = undefined;
    try std.testing.expectEqualStrings("hello world", try conformance.greet(&buf, "world"));
}
