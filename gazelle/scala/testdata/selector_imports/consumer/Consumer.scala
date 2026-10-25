package com.example

import com.example.{Widget, Gadget => G}
import com.example.Tools._
import com.example.Givens.given

object Consumer {
  def parts(): Seq[Any] = Seq(new Widget, new G, ready())
}
