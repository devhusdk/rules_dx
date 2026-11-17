const std = @import("std");
const catalog = @import("catalog");
const digest = @import("digest");
const report = @import("report");

const testing = std.testing;

test "lines list every item and the total" {
    const items = try catalog.parse(testing.allocator, "bulb\t2\t7\nfan\t1\t40\n");
    defer testing.allocator.free(items);
    var buf: [256]u8 = undefined;
    var w: std.Io.Writer = .fixed(&buf);
    try report.lines(&w, items);
    try testing.expectEqualStrings("bulb 2 x 7 = 14\nfan 1 x 40 = 40\ntotal 54\n", w.buffered());
}

test "full report signs the lines with their digest" {
    const items = try catalog.parse(testing.allocator, "bulb\t2\t7\n");
    defer testing.allocator.free(items);
    var buf: [256]u8 = undefined;
    var w: std.Io.Writer = .fixed(&buf);
    try report.full(&w, items);
    const body = "bulb 2 x 7 = 14\ntotal 14\n";
    var want: [256]u8 = undefined;
    const expected = try std.fmt.bufPrint(&want, "{s}schema 2\ndigest {x:0>16}\n", .{ body, digest.of(body) });
    try testing.expectEqualStrings(expected, w.buffered());
}

test "one prints one item" {
    var buf: [64]u8 = undefined;
    var w: std.Io.Writer = .fixed(&buf);
    try report.one(&w, .{ .name = "fan", .quantity = 1, .price = 40 });
    try testing.expectEqualStrings("fan 1 @ 40\n", w.buffered());
}
