/// A length unit.
public enum Unit {
    /// Millimetres.
    case millimetre
    /// Centimetres.
    case centimetre
}

/// A radius in metres.
public typealias Radius = Double

/// Converts a length to metres.
public func metres(_ value: Double, in unit: Unit) -> Double {
    switch unit {
    case .millimetre: return value / 1000
    case .centimetre: return value / 100
    }
}
