import my_project
import XCTest

final class GreetTests: XCTestCase {
  func testGreet() {
    XCTAssertEqual(greeting("world"), "hello world")
  }
}
