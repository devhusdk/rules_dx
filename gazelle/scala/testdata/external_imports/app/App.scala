package com.example.app

import com.example.util.Widget.size
import org.scalactic.{Bad, Good, Or}
import scala.collection.immutable.List
import scala.xml.Elem

object App {
  val sizes: List[Int] = List(size)
  val checked: Int Or String = if (size > 0) Good(size) else Bad("empty")
  def label(node: Elem): String = node.label
}
