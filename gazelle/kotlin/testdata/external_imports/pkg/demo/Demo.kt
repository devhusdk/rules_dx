package demo

import kotlin.collections.List
import kotlin.test.assertEquals
import kotlinx.coroutines.*
import kotlinx.coroutines.runBlocking

class Demo {
  fun run(): List<Int> = runBlocking { listOf(1) }
}
