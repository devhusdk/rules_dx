program io_test
  use iso_fortran_env, only: real64
  implicit none
  integer :: unit, status, length
  character(256) :: message
  character(16) :: text
  character(32) :: value

  message = ""
  open(newunit=unit, file="missing/none.txt", status="old", action="read", iostat=status, iomsg=message)
  if (status == 0) error stop "opening a missing file succeeded"
  if (len_trim(message) == 0) error stop "a failed open has no iomsg"

  write(text, '(f6.3)') 2.5_real64
  if (trim(adjustl(text)) /= "2.500") error stop "internal write of a real64 is wrong"

  call get_environment_variable("DX_FORTRAN_PROBE", value, length, status)
  if (status /= 0 .or. value(1:length) /= "env") error stop "env is not set"

#if DX_FORTRAN_SELECT != 7
  error stop "select() copts did not apply"
#endif
end program io_test
