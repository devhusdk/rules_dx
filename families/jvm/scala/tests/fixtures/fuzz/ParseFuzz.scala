package tests.fixtures.fuzz

object ParseFuzz {
  def fuzzerTestOneInput(data: Array[Byte]): Unit = {
    if (data.length >= 3 && data(0) == 'f' && data(1) == 'u' && data(2) == 'z') {
      throw new IllegalStateException("planted crash")
    }
  }
}
