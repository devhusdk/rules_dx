package platform

import java.time.Instant
import scala.collection.mutable.ListBuffer

object Clock {
  def ticks(): ListBuffer[Instant] = ListBuffer(Instant.now())
}
