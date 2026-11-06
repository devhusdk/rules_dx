import Greet
import XCTest

final class GreetXCTest: XCTestCase {
  func testGreet() {
    XCTAssertEqual(greeting("swift"), "Hello, swift!")
  }
}
