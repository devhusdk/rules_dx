program fail_test
  use greet, only: greeting
  implicit none (type, external)
  if (greeting("world") /= "goodbye world") error stop "failing_case"
end program fail_test
