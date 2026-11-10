program stats_test
  use iso_fortran_env, only: real64
  use stats, only: mean, summary
  implicit none
  real(real64), parameter :: values(4) = [1.0_real64, 2.0_real64, 3.0_real64, 4.0_real64]
  if (abs(mean(values) - 2.5_real64) > 1.0e-12_real64) error stop "mean is wrong"
  if (summary("stats", values) /= "Hello, stats! mean=2.50") error stop "summary is wrong"
end program stats_test
