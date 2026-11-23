module greet
  implicit none (type, external)
  private
  public :: answer
contains
  integer function answer()
    answer = 42
  end function answer
end module greet
