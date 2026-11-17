// Foreign Scala guava helper: one pinned module dep (guava 32.0.1-jre).
package greet

import com.google.common.collect.ImmutableList

object Guava {
  def join(): String = String.join(",", ImmutableList.of("hello", "world"))
}
