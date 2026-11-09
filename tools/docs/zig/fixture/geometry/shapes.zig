//! Named shape sizes.

/// Side of the unit square.
pub const unit: f64 = 1.0;

/// Which kind of shape.
pub const Kind = enum {
    /// Round.
    circle,
    /// Four sides.
    square,
};
