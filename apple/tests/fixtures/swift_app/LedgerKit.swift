import Foundation

public struct Ledger {
  public private(set) var cents: [Int64] = []

  public init?(text: String) {
    for line in text.split(separator: "\n") {
      let trimmed = line.trimmingCharacters(in: .whitespaces)
      if trimmed.isEmpty {
        continue
      }
      let fields = trimmed.split(separator: " ")
      guard fields.count >= 2, let value = Int64(fields[fields.count - 1]) else {
        return nil
      }
      cents.append(value)
    }
  }

  public var total: Int64 {
    return cents.reduce(0, +)
  }

  public mutating func add(cents value: Int64) {
    cents.append(value)
  }

  public func state(platform: String, total: Int64) -> String {
    return "platform=\(platform)\ntotal=\(total)\n"
  }
}
