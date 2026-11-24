const std = @import("std");
const conformance = @import("conformance");

test "greets" {
    var buf: [32]u8 = undefined;
    try std.testing.expectEqualStrings("hello sancov", try conformance.greet(&buf, "sancov"));
}
