program hello
  use iso_fortran_env, only: output_unit
  use greet, only: greeting
  implicit none
  write(output_unit, '(a)') greeting("fortran")
end program hello
