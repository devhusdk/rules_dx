/// A circle.
public struct Circle {
    /// The radius.
    public var radius: Double

    public init(radius: Double) {
        self.radius = radius
    }

    /// Scales the circle.
    ///
    /// - Parameter factor: How much to grow.
    /// - Returns: The grown circle.
    public func scale(by factor: Double) -> Circle {
        Circle(radius: radius * factor)
    }
}

/// Things that have an area.
public protocol Measured {
    /// The area.
    var area: Double { get }
}
