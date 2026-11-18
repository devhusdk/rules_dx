program lock_main
  use testdrive_version, only: get_testdrive_version
  implicit none
  integer :: major, minor, patch

  call get_testdrive_version(major, minor, patch)
  if (major < 0) error stop 1
  print '(a)', 'hello lock'
end program lock_main
