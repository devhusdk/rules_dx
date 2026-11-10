const std = @import("std");
const greet = @import("greet");

pub fn main(init: std.process.Init) !void {
    var buf: [32]u8 = undefined;
    var out_buf: [64]u8 = undefined;
    var out: std.Io.File.Writer = .init(.stdout(), init.io, &out_buf);
    try out.interface.print("{s}\n", .{try greet.greeting(&buf, "zig")});
    try out.interface.flush();
}
