//! Geometry helpers for the Zig docs fixture.

const std = @import("std");

/// Unit shapes, re-exported from their own file.
pub const shapes = @import("shapes.zig");

/// A circle.
pub const Circle = struct {
    /// The radius.
    radius: f64,

    /// The area of the circle.
    pub fn area(self: Circle) f64 {
        return std.math.pi * self.radius * self.radius;
    }

    fn secret(self: Circle) f64 {
        return self.radius;
    }
};

/// Adds two numbers, or fails on overflow.
pub fn add(a: i32, b: i32) !i32 {
    return std.math.add(i32, a, b);
}

/// The origin radius.
pub const origin: f64 = 0.0;

fn hidden() void {}

test "add" {
    try std.testing.expectEqual(@as(i32, 3), try add(1, 2));
}
