import Greet
import XCTest

final class GreetTests: XCTestCase {
  func testGreet() {
    XCTAssertEqual(greeting("world"), "hello world")
  }
}
