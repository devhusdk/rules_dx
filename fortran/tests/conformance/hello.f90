program hello
  use greet, only: greeting
  implicit none
  print '(a)', greeting("world")
end program hello
