const std = @import("std");
const catalog = @import("catalog");
const report = @import("report");

const usage = "usage: manifest [report | get NAME]\n";

pub fn main(init: std.process.Init) !void {
    var out_buf: [4096]u8 = undefined;
    var out: std.Io.File.Writer = .init(.stdout(), init.io, &out_buf);
    var err_buf: [256]u8 = undefined;
    var err: std.Io.File.Writer = .init(.stderr(), init.io, &err_buf);
    const code = try run(init, &out.interface, &err.interface);
    try out.interface.flush();
    try err.interface.flush();
    if (code != 0) std.process.exit(code);
}

fn run(init: std.process.Init, out: *std.Io.Writer, err: *std.Io.Writer) !u8 {
    const args = try init.minimal.args.toSlice(init.arena.allocator());
    const items = try catalog.packaged(init.arena.allocator());
    if (args.len == 1 or (args.len == 2 and std.mem.eql(u8, args[1], "report"))) {
        try report.full(out, items);
        return 0;
    }
    if (args.len == 3 and std.mem.eql(u8, args[1], "get")) {
        const found = catalog.find(items, args[2]) orelse {
            try err.print("manifest: no item {s}\n", .{args[2]});
            return 4;
        };
        try report.one(out, found);
        return 0;
    }
    try err.writeAll(usage);
    return 2;
}
