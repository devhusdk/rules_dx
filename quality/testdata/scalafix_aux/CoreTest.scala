package scalafix_aux

import org.scalatest.flatspec.AnyFlatSpec

class CoreTest extends AnyFlatSpec {
  def box = Core.make()

  "Core.make" should "return a box" in {
    assert(box != null)
  }
}
