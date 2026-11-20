//! Test runner for Zig coverage builds made with -ffuzz.
//! Runs every test once, then writes each sancov PC and whether it ran to $DX_SANCOV_PCS.
const builtin = @import("builtin");
const std = @import("std");
const fuzz_abi = std.Build.abi.fuzz;

const io: std.Io = std.Io.Threaded.global_single_threaded.io();

pub fn main(init: std.process.Init.Minimal) void {
    @disableInstrumentation();
    var failed: usize = 0;
    for (builtin.test_functions) |test_fn| {
        std.testing.allocator_instance = .init(std.heap.page_allocator, .{});
        std.testing.io_instance = .init(std.testing.allocator, .{
            .argv0 = .init(init.args),
            .environ = init.environ,
        });
        std.testing.environ = init.environ;
        std.debug.print("{s}...", .{test_fn.name});
        if (test_fn.func()) |_| {
            std.debug.print("OK\n", .{});
        } else |err| switch (err) {
            error.SkipZigTest => std.debug.print("SKIP\n", .{}),
            else => {
                failed += 1;
                std.debug.print("FAIL ({t})\n", .{err});
                if (@errorReturnTrace()) |trace| std.debug.dumpErrorReturnTrace(trace);
            },
        }
        std.testing.io_instance.deinit();
        if (std.testing.allocator_instance.deinit() != 0) failed += 1;
    }
    const path = init.environ.getAlloc(std.heap.page_allocator, "DX_SANCOV_PCS") catch |err| switch (err) {
        error.EnvironmentVariableMissing => null,
        else => std.debug.panic("cannot read DX_SANCOV_PCS: {t}", .{err}),
    };
    if (path) |p| writePcs(p) catch |err| std.debug.panic("cannot write {s}: {t}", .{ p, err });
    if (failed != 0) std.process.exit(1);
}

/// Writes one "<address> <0|1>" line per instrumented PC, as an address in the file on disk.
/// The counters are 8-bit and wrap, so a block that ran a multiple of 256 times reads as not run.
fn writePcs(path: []const u8) !void {
    @disableInstrumentation();
    const prefix, const suffix = switch (builtin.object_format) {
        .elf => .{ "__start_", "__stop_" },
        .macho => .{ "\x01section$start$__DATA$", "\x01section$end$__DATA$" },
        else => @compileError("no sancov sections for " ++ @tagName(builtin.object_format)),
    };
    const counters_start = @extern([*]u8, .{ .name = prefix ++ "__sancov_cntrs" });
    const counters_end = @extern([*]u8, .{ .name = suffix ++ "__sancov_cntrs" });
    const pcs = @extern([*]usize, .{ .name = prefix ++ "__sancov_pcs1" });
    const counters = counters_start[0 .. counters_end - counters_start];

    const file = try std.Io.Dir.cwd().createFile(io, path, .{});
    defer file.close(io);
    var buffer: [4096]u8 = undefined;
    var writer = file.writer(io, &buffer);
    for (counters, 0..) |count, i| {
        // LLVM leaves placeholder 1 entries in the PC table.
        if (pcs[i] == 1) continue;
        try writer.interface.print("0x{x} {d}\n", .{ fuzz_abi.fuzzer_unslide_address(pcs[i]), @intFromBool(count != 0) });
    }
    try writer.interface.flush();
}

// libfuzzer links against these runner hooks; a coverage run never starts the fuzzer.
export fn runner_test_run(i: u32) void {
    _ = i;
}
export fn runner_test_name(i: u32) fuzz_abi.Slice {
    _ = i;
    return .fromSlice("");
}
export fn runner_broadcast_input(test_i: u32, bytes: fuzz_abi.Slice) void {
    _ = test_i;
    _ = bytes;
}
export fn runner_start_input_poller() void {}
export fn runner_stop_input_poller() void {}
export fn runner_futex_wait(ptr: *const u32, expected: u32) bool {
    _ = ptr;
    _ = expected;
    return false;
}
export fn runner_futex_wake(ptr: *const u32, waiters: u32) void {
    _ = ptr;
    _ = waiters;
}
