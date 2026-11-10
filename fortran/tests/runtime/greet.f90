module greet
  implicit none
  private
  public :: greeting

contains

  function greeting(name) result(text)
    character(*), intent(in) :: name
    character(:), allocatable :: text
    text = "Hello, " // name // "!"
  end function greeting

end module greet
