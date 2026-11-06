package com.example

import com.example.{Widget, Gadget => G}
import com.example.Tools._
import com.example.Givens.given
import _root_.com.example.Widget
import _root_.scala.collection.mutable

object Consumer {
  def parts(): Seq[Any] = Seq(new Widget, new G, ready())
}
