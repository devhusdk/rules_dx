import CDigest

public struct Entry: Equatable, Sendable {
  public let name: String
  public let cents: Int

  public init(name: String, cents: Int) {
    self.name = name
    self.cents = cents
  }
}

public func total(_ entries: [Entry]) -> Int {
  entries.reduce(0) { $0 + $1.cents }
}

public func digest(_ text: String) -> UInt32 {
  let bytes = Array(text.utf8)
  return bytes.withUnsafeBufferPointer { ledger_digest($0.baseAddress, $0.count) }
}
