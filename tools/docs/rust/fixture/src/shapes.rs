//! Plane shapes measured in meters.

/// A circle with a radius in meters.
pub struct Circle {
    /// Radius in meters.
    pub radius: f64,
}

impl Circle {
    /// Returns the area in square meters.
    pub fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }
}

/// Returns the circle with radius one.
#[cfg(feature = "extra")]
pub fn unit_circle() -> Circle {
    Circle { radius: 1.0 }
}

/// Only present without the extra feature.
#[cfg(not(feature = "extra"))]
pub fn without_extra() {}
