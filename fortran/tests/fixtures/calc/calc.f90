module calc
  implicit none
contains
  integer function clamp(value, low, high)
    integer, intent(in) :: value, low, high
    if (value < low) then
      clamp = low
    else if (value > high) then
      clamp = high
    else
      clamp = value
    end if
  end function clamp
end module calc
