program calc_test
  use calc, only: clamp
  implicit none
  if (clamp(-5, 0, 10) /= 0) error stop "low is wrong"
  if (clamp(4, 0, 10) /= 4) error stop "inside is wrong"
  if (clamp(42, 0, 10) /= 10) error stop "high is wrong"
end program calc_test
