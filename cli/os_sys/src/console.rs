#![cfg_attr(
    windows,
    expect(unsafe_code, reason = "console control has no safe std API")
)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtrlEvent {
    C,
    Break,
    Close,
    Logoff,
    Shutdown,
}

impl CtrlEvent {
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<CtrlEvent> {
        match raw {
            0 => Some(CtrlEvent::C),
            1 => Some(CtrlEvent::Break),
            2 => Some(CtrlEvent::Close),
            5 => Some(CtrlEvent::Logoff),
            6 => Some(CtrlEvent::Shutdown),
            _ => None,
        }
    }
}

#[cfg(windows)]
pub use windows::{
    CtrlHandler, add_handler, ignore_ctrl_c, process_ids, restore_ctrl_c, send_ctrl_c,
};

#[cfg(windows)]
pub(crate) mod windows {
    use super::CtrlEvent;
    use std::io;
    use windows_sys::Win32::System::Console::{
        AttachConsole, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent, GetConsoleProcessList,
        SetConsoleCtrlHandler,
    };
    use windows_sys::core::BOOL;

    const LIST_ATTEMPTS: usize = 8;

    pub trait CtrlHandler {
        fn handle(event: CtrlEvent) -> bool;
    }

    pub(crate) extern "system" fn trampoline<H: CtrlHandler>(raw: u32) -> BOOL {
        CtrlEvent::from_raw(raw).map_or(0, |event| BOOL::from(H::handle(event)))
    }

    pub fn add_handler<H: CtrlHandler>() -> io::Result<()> {
        // SAFETY: the trampoline is a static function that lives as long as the process.
        let added = unsafe { SetConsoleCtrlHandler(Some(trampoline::<H>), 1) };
        if added == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        Ok(())
    }

    pub fn restore_ctrl_c() -> io::Result<()> {
        // SAFETY: a null handler with add = 0 only clears the inherited ignore flag.
        let restored = unsafe { SetConsoleCtrlHandler(None, 0) };
        if restored == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        Ok(())
    }

    struct SwallowCtrlC;

    impl CtrlHandler for SwallowCtrlC {
        fn handle(event: CtrlEvent) -> bool {
            event == CtrlEvent::C
        }
    }

    pub fn ignore_ctrl_c() -> io::Result<()> {
        add_handler::<SwallowCtrlC>()
    }

    pub fn send_ctrl_c(pid: u32) -> io::Result<()> {
        // SAFETY: FreeConsole takes no arguments; it fails only when there is no console to leave.
        unsafe { FreeConsole() };
        // SAFETY: AttachConsole takes a plain process id.
        if unsafe { AttachConsole(pid) } == 0 {
            let error = io::Error::last_os_error();
            return Err(io::Error::new(
                error.kind(),
                format!("cannot attach to the console of process {pid}: {error}"),
            ));
        }
        // SAFETY: a null handler with add = 1 only sets this process's ignore-Ctrl-C flag.
        if unsafe { SetConsoleCtrlHandler(None, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GenerateConsoleCtrlEvent takes plain integers; group 0 is the whole console.
        if unsafe { GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) } == 0 {
            let error = io::Error::last_os_error();
            return Err(io::Error::new(
                error.kind(),
                format!("cannot send Ctrl-C to the console of process {pid}: {error}"),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn process_ids() -> Vec<u32> {
        let mut list = vec![0u32; 64];
        for _ in 0..LIST_ATTEMPTS {
            let capacity = u32::try_from(list.len()).unwrap_or(u32::MAX);
            // SAFETY: list is valid for `capacity` u32 entries.
            let count = unsafe { GetConsoleProcessList(list.as_mut_ptr(), capacity) } as usize;
            if count <= list.len() {
                list.truncate(count);
                return list;
                // LCOV_EXCL_START - reason: the list outgrowing its buffer between two calls, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            }
            list.resize(count + 16, 0);
        }
        Vec::new()
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}
