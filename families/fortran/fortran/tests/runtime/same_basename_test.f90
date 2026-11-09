program same_basename_test
  use a_kinds, only: wp, a_scale
  use b_kinds, only: b_scale
  implicit none
  if (abs(a_scale - 2.0_wp) > 1.0e-12_wp) error stop "a_scale is wrong"
  if (abs(b_scale - 6.0_wp) > 1.0e-12_wp) error stop "b_scale is wrong"
end program same_basename_test
