module a_kinds
  use, intrinsic :: iso_fortran_env, only: real64
  implicit none (type, external)
  private
  public :: wp, a_scale
  integer, parameter :: wp = real64
  real(wp), parameter :: a_scale = 2.0_wp
end module a_kinds
