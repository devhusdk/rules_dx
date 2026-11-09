const std = @import("std");

fn parse(input: []const u8) !u32 {
    return std.fmt.parseInt(u32, input, 10);
}

pub fn main(init: std.process.Init) !void {
    const io = init.io;
    const iterations: u64 = 100_000;
    var sink: u32 = 0;
    const start = std.Io.Timestamp.now(io, .awake);
    for (0..iterations) |_| {
        sink +%= try parse("12345");
        std.mem.doNotOptimizeAway(sink);
    }
    const elapsed: i96 = start.untilNow(io, .awake).toNanoseconds();
    const ns_per_op = @as(f64, @floatFromInt(elapsed)) / @as(f64, @floatFromInt(iterations));
    var buf: [128]u8 = undefined;
    const line = try std.fmt.bufPrint(&buf, "bench parse {d:.3} {d}\n", .{ ns_per_op, iterations });
    try std.Io.File.stdout().writeStreamingAll(io, line);
}
