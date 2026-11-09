program calls_c_test
  use iso_c_binding, only: c_int
  implicit none
  interface
    function dx_c_add(a, b) bind(c, name="dx_c_add") result(total)
      import :: c_int
      integer(c_int), value :: a, b
      integer(c_int) :: total
    end function dx_c_add
  end interface
  if (dx_c_add(40_c_int, 2_c_int) /= 42_c_int) error stop "dx_c_add is wrong"
end program calls_c_test
