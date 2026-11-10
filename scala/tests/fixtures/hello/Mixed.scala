// Scala source that calls Java from the same scala_library.
package hello

object Mixed {
  def greet(name: String): String = Greeter.greet(name)
}
