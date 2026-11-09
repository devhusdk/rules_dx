!> Geometry helpers.
module geom
  implicit none
  private
  public :: shape_t, area, pi, circle_area, color
  !> The constant pi.
  real, parameter :: pi = 3.14159
  integer, parameter :: hidden_const = 1
  !> A shape.
  type, abstract :: shape_t
    !> Its name.
    character(len=16) :: name = "x"
    integer, private :: secret
  contains
    !> Computes area.
    procedure(area_i), deferred :: area
  end type
  type, extends(shape_t) :: sq_t
     real :: side
  end type
  abstract interface
    !> area iface
    function area_i(self) result(a)
      import :: shape_t
      class(shape_t), intent(in) :: self
      real :: a
    end function
  end interface
  interface color
    module procedure color_i
  end interface
contains
  !> Area of a circle.
  !! @param r radius
  pure function circle_area(r) result(a)
    !> Radius.
    real, intent(in) :: r
    real :: a
    a = pi*r*r
  end function
  subroutine area(x, y)
    real, intent(in) :: x
    real, intent(out) :: y
    y = x
  end subroutine
  function color_i(c) result(r)
   integer :: c, r
   r = c
  end function
end module geom
!> Main program doc
program main
 use geom
end program
