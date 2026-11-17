module greet

    implicit none (type, external)
    private
    public :: greeting

contains

    function greeting(name) result(text)

        character(*), intent(in)  :: name
        character(:), allocatable :: text

        text = "hello " // name

    end function greeting

end module greet
