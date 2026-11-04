// Foreign Kotlin guava helper: one pinned module dep (guava 32.0.1-jre).
package greet

import com.google.common.collect.ImmutableList

object Guava {
    fun join(): String = ImmutableList.of("hello", "world").joinToString(",")
}
