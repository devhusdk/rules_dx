import LedgerCore
import XCTest

final class LedgerXCTest: XCTestCase {
  func testTotal() {
    let entries = [Entry(name: "tea", cents: 250), Entry(name: "milk", cents: 120)]
    XCTAssertEqual(total(entries), 370)
  }

  func testDigest() {
    XCTAssertEqual(digest("a"), 0xE40C_292C)
  }
}
