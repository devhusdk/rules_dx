use dx_os_sys::job_object::{JobObject, Limit};
use dx_os_sys::win32;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, HandleOrNull, OwnedHandle,
};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSidToSidW, EXPLICIT_ACCESS_W, GetNamedSecurityInfoW, GetSecurityInfo,
    SE_KERNEL_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, SetSecurityInfo, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows_sys::Win32::Security::{
    ACE_HEADER, ACL, DeleteAce, FreeSid, GetAce, GetSecurityDescriptorControl, INHERITED_ACE,
    InitializeSecurityDescriptor, SE_DACL_AUTO_INHERITED, SE_DACL_PROTECTED, SECURITY_CAPABILITIES,
    SECURITY_DESCRIPTOR, SID_AND_ATTRIBUTES, SetFileSecurityW, SetSecurityDescriptorControl,
    SetSecurityDescriptorDacl, TOKEN_ACCESS_MASK, TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess,
    InitializeProcThreadAttributeList, OpenProcessToken, PROCESS_INFORMATION, ReleaseMutex,
    STARTUPINFOEXW, UpdateProcThreadAttribute,
};

use crate::args::{Config, ancestors, command_line, environment, path_key};

const SHARED_ACL_LOCK: &str = "Local\\dx-hermetic-acl";
const ALREADY_EXISTS: i32 = 0x800700B7_u32 as i32;
const SE_GROUP_ENABLED: u32 = 4;
const PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES: usize = 0x0002_0009;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
pub(crate) const CREATE_SUSPENDED: u32 = 0x4;
pub(crate) const CREATE_UNICODE_ENVIRONMENT: u32 = 0x400;
pub(crate) const STARTF_USESTDHANDLES: u32 = 0x100;
const FILE_ALL_ACCESS: u32 = 0x001F_01FF;
// FILE_GENERIC_READ | FILE_TRAVERSE: list a directory and pass through it.
const FILE_LIST_TRAVERSE: u32 = 0x0012_00A9;
const NO_INHERITANCE: u32 = 0;
const SECURITY_DESCRIPTOR_REVISION: u32 = 1;
pub(crate) const GRANT_ACCESS: i32 = 1;
pub(crate) const DENY_ACCESS: i32 = 3;
pub(crate) const REVOKE_ACCESS: i32 = 4;
const SUB_CONTAINERS_AND_OBJECTS_INHERIT: u32 = 3;
const TRUSTEE_IS_SID: i32 = 0;
const TRUSTEE_IS_UNKNOWN: i32 = 0;
const NO_MULTIPLE_TRUSTEE: i32 = 0;
const SE_FILE_OBJECT: i32 = 1;
const DACL_SECURITY_INFORMATION: u32 = 4;
const UNPROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x2000_0000;
const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x8000_0000;
const READ_CONTROL: u32 = 0x0002_0000;
const WRITE_DAC: u32 = 0x0004_0000;
// GetFinalPathNameByHandleW asks the mount manager for drive letters.
const MOUNT_MANAGER: &str = r"\\.\MountPointManager";

// S-1-15-3-1 internetClient, S-1-15-3-3 privateNetworkClientServer.
const NET_CAPABILITIES: [&str; 2] = ["S-1-15-3-1", "S-1-15-3-3"];

pub(crate) fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

pub(crate) fn check(code: u32, what: &str) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("{what} failed with error {code}"))
    }
}

/// Memory a Win32 call allocated for the caller to release with `LocalFree`, freed on drop.
pub(crate) struct LocalMem<T>(*mut T);

impl<T> LocalMem<T> {
    pub(crate) fn null() -> Self {
        Self(null_mut())
    }

    /// The out-pointer for the one call that allocates this memory.
    pub(crate) fn out(&mut self) -> *mut *mut T {
        &mut self.0
    }

    pub(crate) fn get(&self) -> *mut T {
        self.0
    }
}

impl<T> Drop for LocalMem<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from a LocalAlloc-based Win32 call and is freed only here.
            unsafe { LocalFree(self.0 as _) };
        }
    }
}

/// An AppContainer SID, which the profile calls allocate for `FreeSid`.
struct ContainerSid(*mut c_void);

impl Drop for ContainerSid {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the SID came from the AppContainer profile calls and is freed only here.
            unsafe { FreeSid(self.0) };
        }
    }
}

/// An environment block from `CreateEnvironmentBlock`, destroyed on drop.
struct EnvironmentBlock(*mut c_void);

impl Drop for EnvironmentBlock {
    fn drop(&mut self) {
        // SAFETY: the block came from CreateEnvironmentBlock and is destroyed only here.
        unsafe { DestroyEnvironmentBlock(self.0) };
    }
}

/// The number of UTF-16 units before the NUL that ends `text`.
///
/// # Safety
/// `text` must point to a NUL-terminated UTF-16 string.
pub(crate) unsafe fn wide_len(text: *const u16) -> usize {
    let mut len = 0;
    // SAFETY: the caller guarantees a NUL at or after text, so every unit read is in bounds.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    len
}

/// Reads a NUL-terminated UTF-16 string.
///
/// # Safety
/// `text` must point to a NUL-terminated UTF-16 string.
pub(crate) unsafe fn wide_text(text: *const u16) -> String {
    // SAFETY: the caller guarantees text is NUL-terminated.
    let len = unsafe { wide_len(text) };
    // SAFETY: text is valid for len units, as counted above.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) })
}

/// Opens this process's token with `access`.
pub(crate) fn process_token(access: TOKEN_ACCESS_MASK) -> Result<OwnedHandle, String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: GetCurrentProcess takes no arguments and returns a pseudo handle.
    let process = unsafe { GetCurrentProcess() };
    // SAFETY: process is the current process pseudo handle and token is a valid out-pointer.
    if unsafe { OpenProcessToken(process, access, &mut token) } == 0 {
        return Err(format!(
            "cannot open the process token: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: OpenProcessToken succeeded, so token is a new handle owned by the caller.
    Ok(unsafe { OwnedHandle::from_raw_handle(token) })
}

/// The standard input, output and error handles of this process.
pub(crate) fn std_handles() -> [HANDLE; 3] {
    [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE].map(|which| {
        // SAFETY: GetStdHandle takes a plain constant and has no preconditions.
        unsafe { GetStdHandle(which) }
    })
}

/// Takes ownership of the handles a suspended `CreateProcess*W` call returned, puts the
/// process in `job` and resumes it.
pub(crate) fn start_in_job(
    info: &PROCESS_INFORMATION,
    job: &JobObject,
) -> Result<OwnedHandle, String> {
    // SAFETY: hProcess is the new process handle the create call returned, owned only here.
    let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
    // SAFETY: hThread is the new thread handle the create call returned, owned only here.
    let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
    job.assign(process.as_handle())
        .map_err(|err| format!("cannot put the command in the job object: {err}"))?;
    win32::resume_thread(thread.as_handle())
        .map_err(|err| format!("cannot resume the command: {err}"))?;
    Ok(process)
}

/// Waits for `process` to exit and returns its exit code.
pub(crate) fn wait_exit(process: BorrowedHandle<'_>) -> Result<i32, String> {
    win32::wait(process, None).map_err(|err| format!("cannot wait for the command: {err}"))?;
    let code = win32::exit_code(process)
        .map_err(|err| format!("cannot read the command's exit code: {err}"))?;
    Ok(code as i32)
}

/// The user's default environment, used to fill in what the parent lacks.
pub(crate) fn user_defaults() -> Result<Vec<(String, String)>, String> {
    let token = process_token(TOKEN_QUERY | TOKEN_DUPLICATE)?;
    let mut raw: *mut c_void = null_mut();
    // SAFETY: raw is a valid out-pointer and token is an open token handle.
    let created = unsafe { CreateEnvironmentBlock(&mut raw, token.as_raw_handle(), 0) };
    if created == 0 {
        return Err(format!(
            "cannot read the user environment: {}",
            std::io::Error::last_os_error()
        ));
    }
    let block = EnvironmentBlock(raw);
    let mut vars = Vec::new();
    let mut at = block.0 as *const u16;
    loop {
        // SAFETY: at points to an entry of the block, which ends in two NULs.
        let len = unsafe { wide_len(at) };
        if len == 0 {
            break;
        }
        // SAFETY: at is valid for the len units counted above.
        let entry = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(at, len) });
        let split = entry
            .get(1..)
            .and_then(|rest| rest.find('='))
            .map(|at| at + 1);
        if let Some((name, value)) =
            split.and_then(|at| Some((entry.get(..at)?, entry.get(at + 1..)?)))
        {
            vars.push((name.to_owned(), value.to_owned()));
        }
        // SAFETY: the entry's NUL is inside the block, so the next entry starts within it.
        at = unsafe { at.add(len + 1) };
    }
    Ok(vars)
}

fn profile_sid(name: &str) -> Result<ContainerSid, String> {
    let wname = wide(name);
    let mut sid: *mut c_void = null_mut();
    // SAFETY: wname is NUL-terminated and sid is a valid out-pointer.
    let hr = unsafe {
        CreateAppContainerProfile(
            wname.as_ptr(),
            wname.as_ptr(),
            wname.as_ptr(),
            null(),
            0,
            &mut sid,
        )
    };
    if hr == ALREADY_EXISTS {
        // SAFETY: wname is NUL-terminated and sid is a valid out-pointer.
        let hr = unsafe { DeriveAppContainerSidFromAppContainerName(wname.as_ptr(), &mut sid) };
        if hr < 0 {
            return Err(format!("cannot derive the container SID (HRESULT {hr:#x})"));
        }
    } else if hr < 0 {
        return Err(format!(
            "cannot create the container profile (HRESULT {hr:#x})"
        ));
    }
    Ok(ContainerSid(sid))
}

/// Adds (or removes) an inheritable full-access entry for the container on one path.
/// Concurrent qualifications rewrite the same trees, so each path has its own machine-wide lock.
pub(crate) fn set_access(path: &str, sid: *mut c_void, mode: i32) -> Result<(), String> {
    with_acl_lock(&path_lock_name(path), || set_access_locked(path, sid, mode))
}

pub(crate) fn path_lock_name(path: &str) -> String {
    let hash = path_key(path)
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    format!("Global\\dx-hermetic-acl-{hash:016x}")
}

fn set_access_locked(path: &str, sid: *mut c_void, mode: i32) -> Result<(), String> {
    apply_access(path, sid, mode, true)
}

/// Applies `mode` for `sid` to `path`, and with `tree` to everything below it as well.
fn apply_access(path: &str, sid: *mut c_void, mode: i32, tree: bool) -> Result<(), String> {
    let wpath = wide(path);
    let mut old: *mut ACL = null_mut();
    let mut descriptor = LocalMem::<c_void>::null();
    check(
        // SAFETY: wpath is NUL-terminated and old and descriptor are valid out-pointers.
        unsafe {
            GetNamedSecurityInfoW(
                wpath.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old,
                null_mut(),
                descriptor.out(),
            )
        },
        &format!("reading the ACL of {path}"),
    )?;
    let inheritance = if tree || std::path::Path::new(path).is_dir() {
        SUB_CONTAINERS_AND_OBJECTS_INHERIT
    } else {
        NO_INHERITANCE
    };
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: mode,
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid as *mut u16,
        },
    };
    let mut merged = LocalMem::<ACL>::null();

    check(
        // SAFETY: entry is valid, old came from the ACL read and merged is a valid out-pointer.
        unsafe { SetEntriesInAclW(1, &entry, old, merged.out()) },
        "building the ACL",
    )
    .and_then(|()| {
        if tree {
            write_acl(path, descriptor.get(), merged.get(), &|below| {
                apply_access(below, sid, mode, false)
            })
        } else {
            write_acl_here(path, descriptor.get(), merged.get())
        }
    })
}

/// Writes `acl` to `path` and its descendants in the inheritance form of `descriptor`.
/// Where that form is not auto-inherited, `below` makes the same change to each descendant.
pub(crate) fn write_acl(
    path: &str,
    descriptor: *mut c_void,
    acl: *mut ACL,
    below: &dyn Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    let control = dacl_control(path, descriptor)?;
    if control & SE_DACL_AUTO_INHERITED == 0 {
        write_dacl_here(path, control, acl)?;
        return for_each_below(std::path::Path::new(path), below);
    }
    drop_inherited(acl);
    let protection = if control & SE_DACL_PROTECTED != 0 {
        PROTECTED_DACL_SECURITY_INFORMATION
    } else {
        UNPROTECTED_DACL_SECURITY_INFORMATION
    };
    let wpath = wide(path);
    check(
        // SAFETY: wpath is NUL-terminated and acl is a valid ACL.
        unsafe {
            SetNamedSecurityInfoW(
                wpath.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | protection,
                null_mut(),
                null_mut(),
                acl,
                null_mut(),
            )
        },
        &format!("setting the ACL of {path}"),
    )
}

/// Writes `acl` to `path` alone in the inheritance form of `descriptor`.
pub(crate) fn write_acl_here(
    path: &str,
    descriptor: *mut c_void,
    acl: *mut ACL,
) -> Result<(), String> {
    write_dacl_here(path, dacl_control(path, descriptor)?, acl)
}

/// Calls `edit` on every file and directory below `dir`, without following links.
fn for_each_below(
    dir: &std::path::Path,
    edit: &dyn Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    if !std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|err| format!("listing {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("listing {}: {err}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|err| format!("reading {}: {err}", path.display()))?;
        if kind.is_symlink() {
            continue;
        }
        edit(&path.to_string_lossy())?;
        if kind.is_dir() {
            for_each_below(&path, edit)?;
        }
    }
    Ok(())
}

fn dacl_control(path: &str, descriptor: *mut c_void) -> Result<u16, String> {
    let mut control = 0u16;
    let mut revision = 0u32;
    // SAFETY: descriptor is a valid security descriptor and both out-pointers are valid.
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
        return Err(format!(
            "reading the ACL control of {path}: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(control)
}

/// Writes `acl` to `path` alone with the protection and auto-inherit bits of `control`.
fn write_dacl_here(path: &str, control: u16, acl: *mut ACL) -> Result<(), String> {
    let wpath = wide(path);
    // SAFETY: SECURITY_DESCRIPTOR is plain C data for which all-zero is valid.
    let mut fresh: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    let target = &mut fresh as *mut _ as *mut c_void;
    let keep = SE_DACL_PROTECTED | SE_DACL_AUTO_INHERITED;
    // SAFETY: target points to the local, writable security descriptor.
    let ok = unsafe { InitializeSecurityDescriptor(target, SECURITY_DESCRIPTOR_REVISION) } != 0
        // SAFETY: target is initialised and acl is a valid ACL that outlives the descriptor.
        && unsafe { SetSecurityDescriptorDacl(target, 1, acl, 0) } != 0
        // SAFETY: target is an initialised security descriptor.
        && unsafe { SetSecurityDescriptorControl(target, keep, control & keep) } != 0
        // SAFETY: wpath is NUL-terminated and target is a complete security descriptor.
        && unsafe { SetFileSecurityW(wpath.as_ptr(), DACL_SECURITY_INFORMATION, target) } != 0;
    if ok {
        Ok(())
    } else {
        Err(format!(
            "setting the ACL of {path}: {}",
            std::io::Error::last_os_error()
        ))
    }
}

/// Deletes every inherited entry from `acl`, leaving only the explicit ones.
fn drop_inherited(acl: *mut ACL) {
    if acl.is_null() {
        return;
    }
    // SAFETY: acl was checked non-null and points to a valid ACL header.
    for index in (0..unsafe { (*acl).AceCount } as u32).rev() {
        let mut ace: *mut c_void = null_mut();
        // SAFETY: acl is valid, index is below AceCount and ace is a valid out-pointer.
        if unsafe { GetAce(acl, index, &mut ace) } == 0 {
            continue;
        }
        // SAFETY: GetAce returned a valid ACE pointer, which starts with an ACE header.
        let flags = unsafe { (*(ace as *const ACE_HEADER)).AceFlags };
        if u32::from(flags) & INHERITED_ACE != 0 {
            // SAFETY: acl is valid and index is below its AceCount.
            unsafe { DeleteAce(acl, index) };
        }
    }
}

/// An owned named mutex, released on drop.
pub(crate) struct NamedLock(OwnedHandle);

impl NamedLock {
    /// Opens or creates the machine-wide mutex `name` and waits until this process owns it.
    pub(crate) fn acquire(name: &str) -> Result<Self, String> {
        let wname = wide(name);
        // SAFETY: wname is NUL-terminated and outlives the call.
        let raw = unsafe { CreateMutexW(null(), 0, wname.as_ptr()) };
        // SAFETY: CreateMutexW returns a new handle owned by the caller, or null.
        let handle = unsafe { HandleOrNull::from_raw_handle(raw) };
        let mutex = OwnedHandle::try_from(handle).map_err(|error| {
            format!(
                "cannot create the lock {name}: {}: {error}",
                std::io::Error::last_os_error()
            )
        })?;
        win32::wait(mutex.as_handle(), None)
            .map_err(|err| format!("cannot take the lock {name}: {err}"))?;
        Ok(Self(mutex))
    }
}

impl Drop for NamedLock {
    fn drop(&mut self) {
        // SAFETY: the mutex is open and owned by this thread since acquire.
        unsafe { ReleaseMutex(self.0.as_raw_handle()) };
    }
}

/// Holds the named lock while one ACL is read, changed and written back.
pub(crate) fn with_acl_lock<T>(
    name: &str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _lock = NamedLock::acquire(name)?;
    work()
}

/// Adds (or removes) a list-and-traverse entry for the container on one directory only.
/// SetFileSecurityW writes just this directory, so nothing below it is touched.
fn set_traverse(path: &str, sid: *mut c_void, mode: i32) -> Result<(), String> {
    let wpath = wide(path);
    let mut old: *mut ACL = null_mut();
    let mut descriptor = LocalMem::<c_void>::null();
    check(
        // SAFETY: wpath is NUL-terminated and old and descriptor are valid out-pointers.
        unsafe {
            GetNamedSecurityInfoW(
                wpath.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old,
                null_mut(),
                descriptor.out(),
            )
        },
        &format!("reading the ACL of {path}"),
    )?;
    let entry = trustee_entry(sid, FILE_LIST_TRAVERSE, mode);
    let mut merged = LocalMem::<ACL>::null();

    check(
        // SAFETY: entry is valid, old came from the ACL read and merged is a valid out-pointer.
        unsafe { SetEntriesInAclW(1, &entry, old, merged.out()) },
        &format!("building the ACL of {path}"),
    )
    .and_then(|()| write_acl_here(path, descriptor.get(), merged.get()))
}

pub(crate) fn trustee_entry(sid: *mut c_void, access: u32, mode: i32) -> EXPLICIT_ACCESS_W {
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: access,
        grfAccessMode: mode,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid as *mut u16,
        },
    }
}

/// Adds (or removes) a read entry for the container on the mount manager device, so the
/// container can turn volume paths into drive letter paths. It lasts until reboot at most.
fn set_mount_manager(sid: *mut c_void, mode: i32) -> Result<(), String> {
    use std::os::windows::fs::OpenOptionsExt;
    let device = std::fs::OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC)
        .open(MOUNT_MANAGER)
        .map_err(|err| format!("opening {MOUNT_MANAGER}: {err}"))?;
    let handle = device.as_raw_handle();
    let mut old: *mut ACL = null_mut();
    let mut descriptor = LocalMem::<c_void>::null();
    check(
        // SAFETY: handle is the open mount manager device; old and descriptor are out-pointers.
        unsafe {
            GetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old,
                null_mut(),
                descriptor.out(),
            )
        },
        &format!("reading the ACL of {MOUNT_MANAGER}"),
    )?;
    let entry = trustee_entry(sid, FILE_LIST_TRAVERSE, mode);
    let mut merged = LocalMem::<ACL>::null();

    check(
        // SAFETY: entry is valid, old came from the ACL read and merged is a valid out-pointer.
        unsafe { SetEntriesInAclW(1, &entry, old, merged.out()) },
        &format!("building the ACL of {MOUNT_MANAGER}"),
    )
    .and_then(|()| {
        check(
            // SAFETY: handle is the open device handle and merged is the ACL built above.
            unsafe {
                SetSecurityInfo(
                    handle,
                    SE_KERNEL_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    merged.get(),
                    null(),
                )
            },
            &format!("setting the ACL of {MOUNT_MANAGER}"),
        )
    })
}

fn spawn(config: &Config, sid: *mut c_void, job: &JobObject) -> Result<i32, String> {
    let mut sids: Vec<LocalMem<c_void>> = Vec::new();
    if config.net {
        for text in NET_CAPABILITIES {
            let mut cap = LocalMem::null();
            // SAFETY: the wide string outlives the call and cap.out() is a valid out-pointer.
            if unsafe { ConvertStringSidToSidW(wide(text).as_ptr(), cap.out()) } == 0 {
                return Err(format!("cannot convert capability SID {text}"));
            }
            sids.push(cap);
        }
    }
    let mut capabilities: Vec<SID_AND_ATTRIBUTES> = sids
        .iter()
        .map(|cap| SID_AND_ATTRIBUTES {
            Sid: cap.get(),
            Attributes: SE_GROUP_ENABLED,
        })
        .collect();
    let security = SECURITY_CAPABILITIES {
        AppContainerSid: sid,
        Capabilities: if capabilities.is_empty() {
            null_mut()
        } else {
            capabilities.as_mut_ptr()
        },
        CapabilityCount: capabilities.len() as u32,
        Reserved: 0,
    };

    let mut size = 0usize;
    // SAFETY: a null list with a valid size out-pointer only queries the required size.
    unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size) };
    let mut buffer = vec![0u8; size];
    let list = buffer.as_mut_ptr() as *mut c_void;
    // SAFETY: list points to a buffer of the size reported by the query.
    if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut size) } == 0 {
        return Err("cannot initialize the attribute list".to_owned());
    }
    // SAFETY: list is initialised and security outlives the call with the size passed.
    if unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            &security as *const _ as *const c_void,
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
            null_mut(),
            null(),
        )
    } == 0
    {
        return Err("cannot set the security capabilities".to_owned());
    }

    // SAFETY: STARTUPINFOEXW is plain C data for which all-zero is valid.
    let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    [
        startup.StartupInfo.hStdInput,
        startup.StartupInfo.hStdOutput,
        startup.StartupInfo.hStdError,
    ] = std_handles();
    startup.lpAttributeList = list;

    let parent: Vec<(String, String)> = std::env::vars_os()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let (environment, filled) = environment(&parent, &user_defaults()?);
    let mut line = wide(&command_line(&config.command));
    // SAFETY: PROCESS_INFORMATION is plain C data for which all-zero is valid.
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: line is a writable NUL-terminated string; the other pointers are valid for the call.
    let created = unsafe {
        CreateProcessW(
            null(),
            line.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr() as *const c_void,
            null(),
            &startup.StartupInfo,
            &mut info,
        )
    };
    // SAFETY: list was initialised above and is deleted once.
    unsafe { DeleteProcThreadAttributeList(list) };
    drop(sids);
    if created == 0 {
        let err = std::io::Error::last_os_error();
        return Err(format!(
            "cannot start {}: {err} (filled: {})",
            config.command.first().map_or("", String::as_str),
            filled.join(", ")
        ));
    }
    let process = start_in_job(&info, job)?;
    wait_exit(process.as_handle())
}

pub fn run(config: &Config) -> Result<i32, String> {
    let above = ancestors(&config.grants)?;
    let container = profile_sid(&config.name)?;
    let sid = container.0;
    let mut listed = Vec::new();
    let mut granted = Vec::new();
    let mut outcome =
        with_acl_lock(SHARED_ACL_LOCK, || set_mount_manager(sid, GRANT_ACCESS)).map(|()| 1);
    let mounted = outcome.is_ok();
    if outcome.is_ok() {
        for dir in &above {
            if let Err(err) =
                with_acl_lock(SHARED_ACL_LOCK, || set_traverse(dir, sid, GRANT_ACCESS))
            {
                outcome = Err(err);
                break;
            }
            listed.push(dir);
        }
    }
    if outcome.is_ok() {
        for path in &config.grants {
            if let Err(err) = set_access(path, sid, GRANT_ACCESS) {
                outcome = Err(err);
                break;
            }
            granted.push(path);
        }
    }
    if outcome.is_ok() {
        outcome = JobObject::new(Limit::KillOnClose)
            .map_err(|err| format!("cannot create the job object: {err}"))
            .and_then(|job| spawn(config, sid, &job));
    }
    for path in granted {
        let _: Result<(), String> = set_access(path, sid, REVOKE_ACCESS);
    }
    for dir in listed.into_iter().rev() {
        if let Err(err) = with_acl_lock(SHARED_ACL_LOCK, || set_traverse(dir, sid, REVOKE_ACCESS)) {
            eprintln!("appcontainer_run: {err}");
        }
    }
    if mounted
        && let Err(err) = with_acl_lock(SHARED_ACL_LOCK, || set_mount_manager(sid, REVOKE_ACCESS))
    {
        eprintln!("appcontainer_run: {err}");
    }
    drop(container);
    let name = wide(&config.name);
    // SAFETY: name is NUL-terminated and outlives the call.
    let deleted = unsafe { DeleteAppContainerProfile(name.as_ptr()) };
    if deleted < 0 {
        eprintln!("appcontainer_run: cannot delete the container profile (HRESULT {deleted:#x})");
    }
    outcome
}
