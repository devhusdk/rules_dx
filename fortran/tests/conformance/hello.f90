program hello
  use greet, only: greeting
  implicit none (type, external)
  print "(a)", greeting("world")
end program hello
