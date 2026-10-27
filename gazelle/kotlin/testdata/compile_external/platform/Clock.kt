package platform

import java.time.Instant
import kotlin.collections.ArrayDeque

class Clock {
  fun ticks(): ArrayDeque<Instant> = ArrayDeque(listOf(Instant.now()))
}
