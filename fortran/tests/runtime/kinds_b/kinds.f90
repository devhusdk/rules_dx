module b_kinds
  use a_kinds, only: wp, a_scale
  implicit none
  real(wp), parameter :: b_scale = 3.0_wp * a_scale
end module b_kinds
