package com.example.app

import cats.effect.{IO, IOApp}
import com.example.util.Widget.size
import scala.collection.immutable.List

object App {
  val sizes: List[Int] = List(size)
  val io: IO[Int] = IO.pure(size)
}
