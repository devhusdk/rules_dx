const c = @import("c");

pub fn of(bytes: []const u8) u64 {
    return c.manifest_digest(bytes.ptr, bytes.len);
}
