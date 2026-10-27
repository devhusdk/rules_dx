package app

import org.scalactic.{Bad, Good}
import scala.collection.mutable.ListBuffer
import scala.xml.Elem
import util.Widget

object App {
  def render(widget: Widget): Elem = <widget size={widget.size.toString}/>

  def check(size: Int): Any = if (size > 0) Good(size) else Bad(size)

  def all(): ListBuffer[Widget] = ListBuffer(Widget(1))
}
