program lock_main
  use testdrive, only: to_string
  implicit none (type, external)

  if (to_string(7) /= "7") error stop 1
  print "(a)", "hello lock"
end program lock_main
