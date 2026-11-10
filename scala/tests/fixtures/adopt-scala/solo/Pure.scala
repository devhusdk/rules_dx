// Foreign Scala stdlib-only unit: adopted without upstream changes.
package solo

object Pure {
  def pure(name: String): String = "hello " + name
}
