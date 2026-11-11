const std = @import("std");
const conformance = @import("conformance");

test "failing_case" {
    var buf: [32]u8 = undefined;
    try std.testing.expectEqualStrings("goodbye world", try conformance.greet(&buf, "world"));
}
