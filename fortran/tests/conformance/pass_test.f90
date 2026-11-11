program pass_test
  use greet, only: greeting
  implicit none
  if (greeting("world") /= "hello world") error stop 1
end program pass_test
