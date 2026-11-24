program hello
  use, intrinsic :: iso_fortran_env, only: output_unit
  use greet, only: greeting
  implicit none (type, external)
  write(output_unit, "(a)") greeting("fortran")
end program hello
