module a
  use b
  implicit none
contains
  integer function twice()
    twice = 2 * base()
  end function twice
end module a
