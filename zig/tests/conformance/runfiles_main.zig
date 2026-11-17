const std = @import("std");
const runfiles = @import("runfiles");

pub fn main(init: std.process.Init) !void {
    const arena = init.arena.allocator();
    const args = try init.minimal.args.toSlice(arena);
    if (args.len < 2) return error.MissingArgument;
    var r = try runfiles.Runfiles.create(.{
        .allocator = arena,
        .io = init.io,
        .argv0 = args[0],
        .environ_map = init.environ_map,
    }) orelse return error.RunfilesNotFound;
    const path = try r.withSourceRepo("").rlocationAlloc(arena, args[1]) orelse return error.RunfileNotFound;
    const text = try std.Io.Dir.cwd().readFileAlloc(init.io, path, arena, .limited(1 << 20));
    var out_buf: [64]u8 = undefined;
    var out: std.Io.File.Writer = .init(.stdout(), init.io, &out_buf);
    try out.interface.writeAll(text);
    try out.interface.flush();
}
