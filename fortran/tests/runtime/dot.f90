module dot
  use iso_c_binding, only: c_double, c_int
  implicit none
  private
  public :: dx_dot

contains

  function dx_dot(n, a, b) bind(c, name="dx_dot") result(total)
    integer(c_int), value, intent(in) :: n
    real(c_double), intent(in) :: a(n), b(n)
    real(c_double) :: total
    total = dot_product(a, b)
  end function dx_dot

end module dot
