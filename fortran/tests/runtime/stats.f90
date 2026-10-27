module stats
  use iso_fortran_env, only: real64
  use greet, only: greeting
  implicit none
  private
  public :: mean, summary

contains

  pure function mean(values) result(average)
    real(real64), intent(in) :: values(:)
    real(real64) :: average
    average = sum(values) / real(size(values), real64)
  end function mean

  function summary(name, values) result(text)
    character(*), intent(in) :: name
    real(real64), intent(in) :: values(:)
    character(:), allocatable :: text
    character(32) :: buffer
    write(buffer, '(f0.2)') mean(values)
    text = greeting(name) // " mean=" // trim(buffer)
  end function summary

end module stats
