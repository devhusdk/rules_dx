program fail_test
  use greet, only: greeting
  implicit none
  if (greeting("world") /= "goodbye world") error stop "failing_case"
end program fail_test
