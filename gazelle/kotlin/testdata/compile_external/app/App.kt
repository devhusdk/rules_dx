package app

import kotlin.collections.ArrayDeque
import kotlin.reflect.full.memberProperties
import kotlinx.coroutines.runBlocking
import util.Widget

class App {
  fun names(): ArrayDeque<String> = runBlocking {
    ArrayDeque(Widget::class.memberProperties.map { it.name })
  }
}
