const std = @import("std");
const catalog = @import("catalog");
const digest = @import("digest");

pub const Error = std.Io.Writer.Error;

pub fn lines(w: *std.Io.Writer, items: []const catalog.Item) Error!void {
    for (items) |item| {
        try w.print("{s} {d} x {d} = {d}\n", .{ item.name, item.quantity, item.price, item.cost() });
    }
    try w.print("total {d}\n", .{catalog.total(items)});
}

pub fn full(w: *std.Io.Writer, items: []const catalog.Item) Error!void {
    var buf: [4096]u8 = undefined;
    var body: std.Io.Writer = .fixed(&buf);
    try lines(&body, items);
    const text = body.buffered();
    try w.writeAll(text);
    try w.print("schema {d}\n", .{catalog.version});
    try w.print("digest {x:0>16}\n", .{digest.of(text)});
}

pub fn one(w: *std.Io.Writer, found: catalog.Item) Error!void {
    try w.print("{s} {d} @ {d}\n", .{ found.name, found.quantity, found.price });
}
