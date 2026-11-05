import Foundation
import LedgerCore
import LedgerKit
import UIKit
import XCTest

final class LedgerAppTests: XCTestCase {
  func testRustTotalsLedgerLines() {
    XCTAssertEqual(ledger_total("tea 450\nbread 325\n"), 775)
    XCTAssertEqual(ledger_total("broken"), -1)
  }

  func testRustRunsOnTheSimulator() {
    XCTAssertEqual(String(cString: ledger_platform()), "iphonesimulator")
  }

  func testHostAppLoadedTheBundledEntries() throws {
    let delegate = try XCTUnwrap(UIApplication.shared.delegate)
    XCTAssertTrue(NSStringFromClass(type(of: delegate)).contains("LedgerAppDelegate"))
    let path = try XCTUnwrap(Bundle.main.path(forResource: "entries", ofType: "txt"))
    let text = try String(contentsOfFile: path, encoding: .utf8)
    XCTAssertEqual(ledger_total(text), 1375)
    XCTAssertEqual(Ledger(text: text)?.total, 1375)
  }

  func testSwiftAndRustAgreeOnAnAddedEntry() throws {
    let path = try XCTUnwrap(Bundle.main.path(forResource: "entries", ofType: "txt"))
    let text = try String(contentsOfFile: path, encoding: .utf8)
    var ledger = try XCTUnwrap(Ledger(text: text))
    ledger.add(cents: 500)
    XCTAssertEqual(ledger.total, 1875)
    XCTAssertEqual(ledger_total(text + "cake 500\n"), 1875)
    XCTAssertEqual(
      ledger.state(platform: "iphonesimulator", total: ledger.total),
      "platform=iphonesimulator\ntotal=1875\n")
  }
}
