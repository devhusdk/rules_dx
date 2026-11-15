use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSidToSidW, EXPLICIT_ACCESS_W, GetNamedSecurityInfoW, GetSecurityInfo,
    SE_KERNEL_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, SetSecurityInfo, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows_sys::Win32::Security::{
    ACL, GetSecurityDescriptorControl, InitializeSecurityDescriptor, SE_DACL_AUTO_INHERITED,
    SE_DACL_PROTECTED, SECURITY_CAPABILITIES, SECURITY_DESCRIPTOR, SID_AND_ATTRIBUTES,
    SetFileSecurityW, SetSecurityDescriptorControl, SetSecurityDescriptorDacl, TOKEN_DUPLICATE,
    TOKEN_QUERY,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess,
    GetExitCodeProcess, InitializeProcThreadAttributeList, OpenProcessToken, PROCESS_INFORMATION,
    ReleaseMutex, ResumeThread, STARTUPINFOEXW, UpdateProcThreadAttribute, WaitForSingleObject,
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
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
pub(crate) const INFINITE: u32 = 0xFFFF_FFFF;
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

/// The user's default environment, used to fill in what the parent lacks.
pub(crate) fn user_defaults() -> Result<Vec<(String, String)>, String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: token is a valid out-pointer and GetCurrentProcess returns a pseudo handle.
    if unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut token,
        )
    } == 0
    {
        return Err("cannot open the process token".to_owned());
    }
    let mut block: *mut c_void = null_mut();
    // SAFETY: block is a valid out-pointer and token is the open token from above.
    let created = unsafe { CreateEnvironmentBlock(&mut block, token, 0) };
    // SAFETY: token is open and closed once, here.
    unsafe { CloseHandle(token) };
    if created == 0 {
        return Err(format!(
            "cannot read the user environment: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut vars = Vec::new();
    let mut at = block as *const u16;
    // SAFETY: block came from CreateEnvironmentBlock, is double-NUL terminated and destroyed once.
    unsafe {
        loop {
            let mut len = 0;
            while *at.add(len) != 0 {
                len += 1;
            }
            if len == 0 {
                break;
            }
            let entry = String::from_utf16_lossy(std::slice::from_raw_parts(at, len));
            let split = entry
                .get(1..)
                .and_then(|rest| rest.find('='))
                .map(|at| at + 1);
            if let Some((name, value)) =
                split.and_then(|at| Some((entry.get(..at)?, entry.get(at + 1..)?)))
            {
                vars.push((name.to_owned(), value.to_owned()));
            }
            at = at.add(len + 1);
        }
        DestroyEnvironmentBlock(block);
    }
    Ok(vars)
}

fn profile_sid(name: &str) -> Result<*mut c_void, String> {
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
    Ok(sid)
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
    let wpath = wide(path);
    let mut old: *mut ACL = null_mut();
    let mut descriptor: *mut c_void = null_mut();
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
                &mut descriptor,
            )
        },
        &format!("reading the ACL of {path}"),
    )?;
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: mode,
        grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid as *mut u16,
        },
    };
    let mut merged: *mut ACL = null_mut();
    let result = check(
        // SAFETY: entry is valid, old came from the ACL read and merged is a valid out-pointer.
        unsafe { SetEntriesInAclW(1, &entry, old, &mut merged) },
        "building the ACL",
    )
    .and_then(|()| {
        check(
            // SAFETY: wpath is NUL-terminated and merged is the ACL built above.
            unsafe {
                SetNamedSecurityInfoW(
                    wpath.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    merged,
                    null_mut(),
                )
            },
            &format!("setting the ACL of {path}"),
        )
    });
    // SAFETY: merged and descriptor were allocated by the ACL calls above and are freed once.
    unsafe {
        LocalFree(merged as _);
        LocalFree(descriptor as _);
    }
    result
}

/// Holds the named lock while one ACL is read, changed and written back.
pub(crate) fn with_acl_lock<T>(
    name: &str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    // SAFETY: the wide name outlives the call and a null mutex is handled below.
    let mutex = unsafe { CreateMutexW(null(), 0, wide(name).as_ptr()) };
    if mutex.is_null() {
        return Err(format!(
            "cannot create the ACL lock: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: mutex is the valid handle created above.
    unsafe { WaitForSingleObject(mutex, INFINITE) };
    let result = work();
    // SAFETY: mutex is owned here, released and closed once.
    unsafe {
        ReleaseMutex(mutex);
        CloseHandle(mutex);
    }
    result
}

/// Adds (or removes) a list-and-traverse entry for the container on one directory only.
/// SetFileSecurityW writes just this directory, so nothing below it is touched.
fn set_traverse(path: &str, sid: *mut c_void, mode: i32) -> Result<(), String> {
    let wpath = wide(path);
    let mut old: *mut ACL = null_mut();
    let mut descriptor: *mut c_void = null_mut();
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
                &mut descriptor,
            )
        },
        &format!("reading the ACL of {path}"),
    )?;
    let entry = trustee_entry(sid, FILE_LIST_TRAVERSE, mode);
    let mut merged: *mut ACL = null_mut();
    let result = check(
        // SAFETY: entry is valid, old came from the ACL read and merged is a valid out-pointer.
        unsafe { SetEntriesInAclW(1, &entry, old, &mut merged) },
        &format!("building the ACL of {path}"),
    )
    .and_then(|()| {
        let mut control = 0u16;
        let mut revision = 0u32;
        // SAFETY: SECURITY_DESCRIPTOR is plain C data for which all-zero is valid.
        let mut fresh: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
        let target = &mut fresh as *mut _ as *mut c_void;
        let keep = SE_DACL_PROTECTED | SE_DACL_AUTO_INHERITED;
        // SAFETY: descriptor was read above and target points to the local security descriptor.
        let ok = unsafe {
            GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) != 0
                && InitializeSecurityDescriptor(target, SECURITY_DESCRIPTOR_REVISION) != 0
                && SetSecurityDescriptorDacl(target, 1, merged, 0) != 0
                && SetSecurityDescriptorControl(target, keep, control & keep) != 0
                && SetFileSecurityW(wpath.as_ptr(), DACL_SECURITY_INFORMATION, target) != 0
        };
        if ok {
            Ok(())
        } else {
            Err(format!(
                "setting the ACL of {path}: {}",
                std::io::Error::last_os_error()
            ))
        }
    });
    // SAFETY: merged and descriptor were allocated by the ACL calls above and are freed once.
    unsafe {
        LocalFree(merged as _);
        LocalFree(descriptor as _);
    }
    result
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
    use std::os::windows::io::AsRawHandle;
    let device = std::fs::OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC)
        .open(MOUNT_MANAGER)
        .map_err(|err| format!("opening {MOUNT_MANAGER}: {err}"))?;
    let handle = device.as_raw_handle() as HANDLE;
    let mut old: *mut ACL = null_mut();
    let mut descriptor: *mut c_void = null_mut();
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
                &mut descriptor,
            )
        },
        &format!("reading the ACL of {MOUNT_MANAGER}"),
    )?;
    let entry = trustee_entry(sid, FILE_LIST_TRAVERSE, mode);
    let mut merged: *mut ACL = null_mut();
    let result = check(
        // SAFETY: entry is valid, old came from the ACL read and merged is a valid out-pointer.
        unsafe { SetEntriesInAclW(1, &entry, old, &mut merged) },
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
                    merged,
                    null(),
                )
            },
            &format!("setting the ACL of {MOUNT_MANAGER}"),
        )
    });
    // SAFETY: merged and descriptor were allocated by the ACL calls above and are freed once.
    unsafe {
        LocalFree(merged as _);
        LocalFree(descriptor as _);
    }
    result
}

pub(crate) fn kill_on_close_job() -> Result<HANDLE, String> {
    // SAFETY: CreateJobObjectW accepts null attributes and name; a null result is checked.
    let job = unsafe { CreateJobObjectW(null(), null()) };
    if job.is_null() {
        return Err("cannot create the job object".to_owned());
    }
    // SAFETY: the extended limit structure is plain C data for which all-zero is valid.
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: job is valid and info is an extended limit structure of the size passed.
    let ok = unsafe {
        SetInformationJobObject(
            job,
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
            &info as *const _ as *const c_void,
            std::mem::size_of_val(&info) as u32,
        )
    };
    if ok == 0 {
        return Err("cannot configure the job object".to_owned());
    }
    Ok(job)
}

fn spawn(config: &Config, sid: *mut c_void, job: HANDLE) -> Result<i32, String> {
    let mut capabilities: Vec<SID_AND_ATTRIBUTES> = Vec::new();
    if config.net {
        for text in NET_CAPABILITIES {
            let mut cap: *mut c_void = null_mut();
            // SAFETY: the wide string outlives the call and cap is a valid out-pointer.
            if unsafe { ConvertStringSidToSidW(wide(text).as_ptr(), &mut cap) } == 0 {
                return Err(format!("cannot convert capability SID {text}"));
            }
            capabilities.push(SID_AND_ATTRIBUTES {
                Sid: cap,
                Attributes: SE_GROUP_ENABLED,
            });
        }
    }
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
    // SAFETY: GetStdHandle takes plain constants and has no preconditions.
    unsafe {
        startup.StartupInfo.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
        startup.StartupInfo.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
        startup.StartupInfo.hStdError = GetStdHandle(STD_ERROR_HANDLE);
    }
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
    for cap in &capabilities {
        // SAFETY: cap.Sid was allocated by ConvertStringSidToSidW and is freed once.
        unsafe { LocalFree(cap.Sid as _) };
    }
    if created == 0 {
        let err = std::io::Error::last_os_error();
        return Err(format!(
            "cannot start {}: {err} (filled: {})",
            config.command.first().map_or("", String::as_str),
            filled.join(", ")
        ));
    }
    // SAFETY: job and info.hProcess are valid handles.
    if unsafe { AssignProcessToJobObject(job, info.hProcess) } == 0 {
        return Err("cannot put the command in the job object".to_owned());
    }
    // SAFETY: info holds the live handles from CreateProcess; hThread is closed once.
    unsafe {
        ResumeThread(info.hThread);
        CloseHandle(info.hThread);
        WaitForSingleObject(info.hProcess, INFINITE);
    }
    let mut code = 1u32;
    // SAFETY: info.hProcess is live, code is a valid out-pointer and the handle is closed once.
    unsafe {
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
    }
    Ok(code as i32)
}

pub fn run(config: &Config) -> Result<i32, String> {
    let above = ancestors(&config.grants)?;
    let sid = profile_sid(&config.name)?;
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
        outcome = kill_on_close_job().and_then(|job| {
            let code = spawn(config, sid, job);
            // SAFETY: job is the valid handle from kill_on_close_job, closed once here.
            unsafe { CloseHandle(job) };
            code
        });
    }
    for path in granted {
        let _ = set_access(path, sid, REVOKE_ACCESS);
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
    // SAFETY: the profile name outlives the call and sid is freed once.
    unsafe {
        DeleteAppContainerProfile(wide(&config.name).as_ptr());
        LocalFree(sid);
    }
    outcome
}
