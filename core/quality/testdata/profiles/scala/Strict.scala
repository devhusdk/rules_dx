package fixtures.profiles

object Strict {
  private val unused = 1

  def greet = Seq(1, 2).sum

  def guarded(xs: Array[Any]): String = {
    var n = 0
    while (n < xs.length) n += 1
    val Some(first) = xs.headOption
    if (n == 0) throw new IllegalStateException(s"$first")
    first.asInstanceOf[String] + null
  }
}
