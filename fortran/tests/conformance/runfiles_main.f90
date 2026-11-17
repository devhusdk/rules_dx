program runfiles_main

    implicit none (type, external)

    character(len=4096) :: rpath, dir, manifest, path, line
    integer             :: length, status, unit, ios

    call get_command_argument(1, rpath, length, status)
    if (status /= 0) error stop "missing runfiles path"
    call get_environment_variable("RUNFILES_DIR", dir, length, status)
    if (status == 0 .and. length > 0) then
        path = trim(dir) // "/" // trim(rpath)
    else
        call get_environment_variable("RUNFILES_MANIFEST_FILE", manifest, length, status)
        if (status /= 0) error stop "no runfiles"
        path = ""
        open (newunit=unit, file=trim(manifest), status="old", action="read")
        do
            read (unit, "(a)", iostat=ios) line
            if (ios /= 0) exit
            if (index(line, trim(rpath) // " ") == 1) then
                path = adjustl(line(len_trim(rpath) + 2:))
                length = len_trim(path)
                if (length > 0 .and. path(length:length) == achar(13)) path(length:length) = " "
                exit
            end if
        end do
        close (unit)
        if (len_trim(path) == 0) error stop "runfile not in manifest"
    end if

    open (newunit=unit, file=trim(path), status="old", action="read")
    do
        read (unit, "(a)", iostat=ios) line
        if (ios /= 0) exit
        print "(a)", trim(line)
    end do
    close (unit)
end program runfiles_main
