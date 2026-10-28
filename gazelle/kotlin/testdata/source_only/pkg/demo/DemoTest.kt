package demo

class DemoTest {
  @Test
  fun testGreet() {
    assert(Demo.greet("world") == "hello world")
  }
}
