//! The user backend: a new local account per run, its network blocked by WFP filters. The
//! account is also denied the DNS Client service's RPC port, so it cannot resolve names through
//! a service that queries on its behalf.
//!
//! With `--group`, the account joins a local group that `setup_group` gave the grants and
//! denies once, so a run does not rewrite the inherited ACLs of whole trees twice. Before that,
//! `setup_group` removes the `dxh-<pid>` groups of runs that were killed before their teardown,
//! with their entries on this run's paths.
//!
//! The denies go on the shared `dxh-host` group instead, which every run account joins. Its
//! entries stay between runs: `ensure_host_denies` walks a tree only when the host deny list
//! does not name it yet, so the big Visual Studio and SDK trees are rewritten once per machine.
//!
//! Inside an AppContainer the Mount Manager refuses `GetFinalPathNameByHandleW` with a DOS
//! volume name, which Bazel's client needs. A normal user token passes.
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_INVALID_PARAMETER, ERROR_MORE_DATA,
    ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, LUID, LocalFree, WAIT_TIMEOUT,
};
use windows_sys::Win32::NetworkManagement::NetManagement::{
    LOCALGROUP_INFO_0, LOCALGROUP_MEMBERS_INFO_3, MAX_PREFERRED_LENGTH, NetApiBufferFree,
    NetLocalGroupAdd, NetLocalGroupAddMembers, NetLocalGroupDel, NetLocalGroupEnum, NetUserAdd,
    NetUserDel, UF_DONT_EXPIRE_PASSWD, UF_PASSWD_CANT_CHANGE, UF_SCRIPT, USER_INFO_1,
    USER_PRIV_USER,
};
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FWP_ACTION_BLOCK, FWP_ACTION_PERMIT, FWP_BYTE_BLOB, FWP_CONDITION_FLAG_IS_LOOPBACK,
    FWP_CONDITION_VALUE0, FWP_CONDITION_VALUE0_0, FWP_MATCH_EQUAL, FWP_MATCH_FLAGS_ALL_SET,
    FWP_SECURITY_DESCRIPTOR_TYPE, FWP_UINT8, FWP_UINT16, FWP_UINT32, FWP_VALUE0, FWP_VALUE0_0,
    FWPM_CONDITION_ALE_USER_ID, FWPM_CONDITION_FLAGS, FWPM_CONDITION_IP_REMOTE_PORT,
    FWPM_FILTER_CONDITION0, FWPM_FILTER0, FWPM_LAYER_ALE_AUTH_CONNECT_V4,
    FWPM_LAYER_ALE_AUTH_CONNECT_V6, FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
    FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6, FWPM_SESSION_FLAG_DYNAMIC, FWPM_SESSION0, FwpmEngineClose0,
    FwpmEngineOpen0, FwpmFilterAdd0,
};
use windows_sys::Win32::Security::Authentication::Identity::RtlGenRandom;
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    GetNamedSecurityInfoW, GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT, SE_KERNEL_OBJECT,
    SetEntriesInAclW, SetNamedSecurityInfoW, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, AdjustTokenPrivileges, DACL_SECURITY_INFORMATION, DeleteAce, EqualSid,
    GetAce, GetSecurityDescriptorLength, INHERITED_ACE, LUID_AND_ATTRIBUTES, LookupAccountNameW,
    LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, CreateProcessWithLogonW, GetCurrentProcess, GetExitCodeProcess,
    LOGON_WITH_PROFILE, OpenProcess, OpenProcessToken, PROCESS_DUP_HANDLE, PROCESS_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, ReleaseMutex, ResumeThread, STARTUPINFOW,
    WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::DeleteProfileW;
use windows_sys::core::GUID;

use crate::args::{
    Config, HOST_GROUP, Mode, acl_waves, environment, logon_command_line, password, path_key,
    stale_groups, user_name,
};
use crate::sys::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, DENY_ACCESS, GRANT_ACCESS, INFINITE,
    REVOKE_ACCESS, STARTF_USESTDHANDLES, check, kill_on_close_job, path_lock_name, set_access,
    trustee_entry, user_defaults, wide, with_acl_lock,
};

const RPC_C_AUTHN_DEFAULT: u32 = 0xFFFF_FFFF;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_MEMBER_IN_ALIAS: u32 = 1378;
const ERROR_ALIAS_EXISTS: u32 = 1379;
const BLOCK_WEIGHT: u8 = 1;
const PERMIT_WEIGHT: u8 = 10;
const DNS_BLOCK_WEIGHT: u8 = 15;
const DNS_PORT: u16 = 53;
const PORT_ALL_ACCESS: u32 = 0x001F_0001;
const SYSTEM_EXTENDED_HANDLE_INFORMATION: u32 = 64;
const OBJECT_NAME_INFORMATION: u32 = 1;
const OBJECT_TYPE_INFORMATION: u32 = 2;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
const SC_MANAGER_CONNECT: u32 = 1;
const SERVICE_QUERY_STATUS: u32 = 4;
const SC_STATUS_PROCESS_INFO: i32 = 0;
const DNS_PORT_LOCK: &str = "Global\\dx_hermetic_dnsresolver_acl";
const HOST_DENIES_LOCK: &str = "Global\\dx-hermetic-host-denies";
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;

#[repr(C)]
struct HandleEntry {
    object: *mut c_void,
    pid: usize,
    handle: usize,
    access: u32,
    backtrace: u16,
    type_index: u16,
    attributes: u32,
    reserved: u32,
}

#[repr(C)]
struct UnicodeString {
    length: u16,
    capacity: u16,
    buffer: *const u16,
}

#[link(name = "ntdll", kind = "raw-dylib")]
// SAFETY: the declarations match the ntdll signatures of these functions.
unsafe extern "system" {
    fn NtQuerySystemInformation(class: u32, info: *mut c_void, len: u32, out: *mut u32) -> i32;
    fn NtQueryObject(handle: HANDLE, class: u32, info: *mut c_void, len: u32, out: *mut u32)
    -> i32;
}

#[link(name = "advapi32", kind = "raw-dylib")]
// SAFETY: the declarations match the advapi32 signatures of these functions.
unsafe extern "system" {
    fn OpenSCManagerW(machine: *const u16, database: *const u16, access: u32) -> HANDLE;
    fn OpenServiceW(manager: HANDLE, name: *const u16, access: u32) -> HANDLE;
    fn QueryServiceStatusEx(
        service: HANDLE,
        level: i32,
        buffer: *mut u8,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn CloseServiceHandle(handle: HANDLE) -> i32;
}

/// Everything a run creates. Dropping it undoes whatever exists, in a fixed order.
struct Session {
    user: Vec<u16>,
    created: bool,
    sid: Vec<u8>,
    sid_text: String,
    job: HANDLE,
    acl_paths: Vec<String>,
    engine: HANDLE,
    dns_port: HANDLE,
    dns_pid: u32,
}

impl Session {
    fn sid(&self) -> *mut c_void {
        self.sid.as_ptr() as *mut c_void
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.job.is_null() {
            end_job(self.job);
        }
        for path in &self.acl_paths {
            let _ = remove_access(path, self.sid());
        }
        if !self.dns_port.is_null() {
            let _ = set_port_access(self.dns_port, self.sid(), REVOKE_ACCESS);
            // SAFETY: dns_port is the duplicated handle this session owns, closed once here.
            unsafe { CloseHandle(self.dns_port) };
        }
        if self.created {
            if !self.sid_text.is_empty() {
                delete_profile(&self.sid_text);
            }
            // SAFETY: user is a NUL-terminated wide name and null selects the local server.
            unsafe { NetUserDel(null(), self.user.as_ptr()) };
        }
        if !self.engine.is_null() {
            // SAFETY: engine is the open filtering engine handle, closed once here.
            unsafe { FwpmEngineClose0(self.engine) };
        }
    }
}

/// Kills what is left in the job and waits until it is empty, so the profile is no longer in use.
fn end_job(job: HANDLE) {
    // SAFETY: the accounting structure is plain C data for which all-zero is valid.
    let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: job is a valid job handle.
    unsafe { TerminateJobObject(job, 1) };
    for _ in 0..100 {
        // SAFETY: job is valid and info is the accounting structure of the size passed.
        let ok = unsafe {
            QueryInformationJobObject(
                job,
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut c_void,
                std::mem::size_of_val(&info) as u32,
                null_mut(),
            )
        };
        if ok == 0 || info.ActiveProcesses == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // SAFETY: job is owned here and closed once.
    unsafe { CloseHandle(job) };
}

/// Deletes the account's profile. The profile service unloads it a little after the last
/// process exits and refuses to delete it until then, so a profile left behind on every
/// run would fill the system disk.
fn delete_profile(sid: &str) {
    let sid = wide(sid);
    let mut err = None;
    for _ in 0..60 {
        // SAFETY: sid is a NUL-terminated wide string and null selects the local machine.
        if unsafe { DeleteProfileW(sid.as_ptr(), null(), null()) } != 0 {
            return;
        }
        let last = std::io::Error::last_os_error();
        if last.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) {
            return;
        }
        err = Some(last);
        std::thread::sleep(Duration::from_millis(250));
    }
    if let Some(err) = err {
        eprintln!("appcontainer_run: cannot delete the run account's profile: {err}");
    }
}

fn random_password() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    // SAFETY: bytes is a valid buffer of the length passed.
    if !unsafe { RtlGenRandom(bytes.as_mut_ptr() as *mut c_void, bytes.len() as u32) } {
        return Err("cannot get random bytes for the account password".to_owned());
    }
    Ok(password(&bytes))
}

fn create_user(session: &mut Session, password: &str) -> Result<(), String> {
    let mut password = wide(password);
    let mut comment = wide("dx hermetic run");
    let info = USER_INFO_1 {
        usri1_name: session.user.as_mut_ptr(),
        usri1_password: password.as_mut_ptr(),
        usri1_password_age: 0,
        usri1_priv: USER_PRIV_USER,
        usri1_home_dir: null_mut(),
        usri1_comment: comment.as_mut_ptr(),
        usri1_flags: UF_SCRIPT | UF_DONT_EXPIRE_PASSWD | UF_PASSWD_CANT_CHANGE,
        usri1_script_path: null_mut(),
    };
    let mut bad = 0u32;
    // SAFETY: info is a valid USER_INFO_1 whose strings outlive the call; bad is an out-pointer.
    let code = unsafe { NetUserAdd(null(), 1, &info as *const _ as *const u8, &mut bad) };
    if code == ERROR_ACCESS_DENIED {
        return Err(
            "NetUserAdd failed with error 5: the user backend must run elevated (as administrator)"
                .to_owned(),
        );
    }
    check(code, "creating the local account")?;
    session.created = true;
    add_member(
        &session.user,
        "Users",
        "adding the account to the Users group (it needs the local logon right)",
    )
}

fn add_member(user: &[u16], group: &str, what: &str) -> Result<(), String> {
    let mut member = user.to_vec();
    let info = LOCALGROUP_MEMBERS_INFO_3 {
        lgrmi3_domainandname: member.as_mut_ptr(),
    };
    // SAFETY: info outlives the call and holds a member name, as level 3 requires.
    let code = unsafe {
        NetLocalGroupAddMembers(
            null(),
            wide(group).as_ptr(),
            3,
            &info as *const _ as *const u8,
            1,
        )
    };
    if code == ERROR_MEMBER_IN_ALIAS {
        return Ok(());
    }
    check(code, what)
}

fn account_sid(name: &[u16]) -> Result<Vec<u8>, String> {
    let (mut sid_len, mut domain_len, mut kind) = (0u32, 0u32, 0i32);
    // SAFETY: name is NUL-terminated and the null buffers only request the required sizes.
    unsafe {
        LookupAccountNameW(
            null(),
            name.as_ptr(),
            null_mut(),
            &mut sid_len,
            null_mut(),
            &mut domain_len,
            &mut kind,
        )
    };
    let mut sid = vec![0u8; sid_len as usize];
    let mut domain = vec![0u16; domain_len as usize];
    // SAFETY: sid and domain are buffers of the lengths passed and name is NUL-terminated.
    let found = unsafe {
        LookupAccountNameW(
            null(),
            name.as_ptr(),
            sid.as_mut_ptr() as *mut c_void,
            &mut sid_len,
            domain.as_mut_ptr(),
            &mut domain_len,
            &mut kind,
        )
    };
    if found == 0 {
        return Err(format!(
            "cannot look up the SID of {}: {}",
            String::from_utf16_lossy(name.strip_suffix(&[0]).unwrap_or(name)),
            std::io::Error::last_os_error()
        ));
    }
    Ok(sid)
}

fn look_up_sid(session: &mut Session) -> Result<(), String> {
    session.sid = account_sid(&session.user)?;
    let mut text: *mut u16 = null_mut();
    // SAFETY: session.sid() is a valid SID and text is a valid out-pointer.
    if unsafe { ConvertSidToStringSidW(session.sid(), &mut text) } == 0 {
        return Err("cannot print the account SID".to_owned());
    }
    // SAFETY: text is a NUL-terminated string from ConvertSidToStringSidW, freed once.
    unsafe {
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        session.sid_text = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        LocalFree(text as _);
    }
    Ok(())
}

fn add_filter(
    engine: HANDLE,
    layer: GUID,
    action: u32,
    weight: u8,
    conditions: &mut [FWPM_FILTER_CONDITION0],
) -> Result<(), String> {
    let mut name = wide("dx hermetic");
    // SAFETY: FWPM_FILTER0 is plain C data for which all-zero is valid.
    let mut filter: FWPM_FILTER0 = unsafe { std::mem::zeroed() };
    filter.displayData.name = name.as_mut_ptr();
    filter.layerKey = layer;
    filter.weight = FWP_VALUE0 {
        r#type: FWP_UINT8,
        Anonymous: FWP_VALUE0_0 { uint8: weight },
    };
    filter.numFilterConditions = conditions.len() as u32;
    filter.filterCondition = conditions.as_mut_ptr();
    filter.action.r#type = action;
    let mut id = 0u64;
    check(
        // SAFETY: engine is open and filter, whose pointers outlive the call, is initialised.
        unsafe { FwpmFilterAdd0(engine, &filter, null_mut(), &mut id) },
        "adding a network filter",
    )
}

/// Blocks every connect and accept of the account except loopback, until the engine closes.
fn block_network(session: &mut Session) -> Result<(), String> {
    // SAFETY: FWPM_SESSION0 is plain C data for which all-zero is valid.
    let mut wfp: FWPM_SESSION0 = unsafe { std::mem::zeroed() };
    let mut label = wide("dx hermetic");
    wfp.displayData.name = label.as_mut_ptr();
    wfp.flags = FWPM_SESSION_FLAG_DYNAMIC;
    check(
        // SAFETY: wfp and session.engine are valid and label outlives the call.
        unsafe {
            FwpmEngineOpen0(
                null(),
                RPC_C_AUTHN_DEFAULT,
                null(),
                &wfp,
                &mut session.engine,
            )
        },
        "opening the filtering engine",
    )?;

    let sddl = wide(&format!("O:LSD:(A;;CC;;;{})", session.sid_text));
    let mut descriptor: *mut c_void = null_mut();
    // SAFETY: sddl is NUL-terminated and descriptor is a valid out-pointer.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err("cannot build the account match for the network filters".to_owned());
    }
    let mut blob = FWP_BYTE_BLOB {
        // SAFETY: descriptor was just returned by the conversion above.
        size: unsafe { GetSecurityDescriptorLength(descriptor) },
        data: descriptor as *mut u8,
    };
    let blob_ptr: *mut FWP_BYTE_BLOB = &mut blob;
    let for_user = || {
        // SAFETY: FWPM_FILTER_CONDITION0 is plain C data for which all-zero is valid.
        let mut condition: FWPM_FILTER_CONDITION0 = unsafe { std::mem::zeroed() };
        condition.fieldKey = FWPM_CONDITION_ALE_USER_ID;
        condition.matchType = FWP_MATCH_EQUAL;
        condition.conditionValue = FWP_CONDITION_VALUE0 {
            r#type: FWP_SECURITY_DESCRIPTOR_TYPE,
            Anonymous: FWP_CONDITION_VALUE0_0 { sd: blob_ptr },
        };
        condition
    };
    let loopback = || {
        // SAFETY: FWPM_FILTER_CONDITION0 is plain C data for which all-zero is valid.
        let mut condition: FWPM_FILTER_CONDITION0 = unsafe { std::mem::zeroed() };
        condition.fieldKey = FWPM_CONDITION_FLAGS;
        condition.matchType = FWP_MATCH_FLAGS_ALL_SET;
        condition.conditionValue = FWP_CONDITION_VALUE0 {
            r#type: FWP_UINT32,
            Anonymous: FWP_CONDITION_VALUE0_0 {
                uint32: FWP_CONDITION_FLAG_IS_LOOPBACK,
            },
        };
        condition
    };
    let dns = || {
        // SAFETY: FWPM_FILTER_CONDITION0 is plain C data for which all-zero is valid.
        let mut condition: FWPM_FILTER_CONDITION0 = unsafe { std::mem::zeroed() };
        condition.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
        condition.matchType = FWP_MATCH_EQUAL;
        condition.conditionValue = FWP_CONDITION_VALUE0 {
            r#type: FWP_UINT16,
            Anonymous: FWP_CONDITION_VALUE0_0 { uint16: DNS_PORT },
        };
        condition
    };
    let mut result = Ok(());
    // A local resolver must stay out of reach, so port 53 is blocked above the loopback permit.
    for layer in [
        FWPM_LAYER_ALE_AUTH_CONNECT_V4,
        FWPM_LAYER_ALE_AUTH_CONNECT_V6,
    ] {
        result = result.and_then(|()| {
            add_filter(
                session.engine,
                layer,
                FWP_ACTION_BLOCK,
                DNS_BLOCK_WEIGHT,
                &mut [for_user(), dns()],
            )
        });
    }
    for layer in [
        FWPM_LAYER_ALE_AUTH_CONNECT_V4,
        FWPM_LAYER_ALE_AUTH_CONNECT_V6,
        FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
        FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
    ] {
        result = result
            .and_then(|()| {
                add_filter(
                    session.engine,
                    layer,
                    FWP_ACTION_BLOCK,
                    BLOCK_WEIGHT,
                    &mut [for_user()],
                )
            })
            .and_then(|()| {
                add_filter(
                    session.engine,
                    layer,
                    FWP_ACTION_PERMIT,
                    PERMIT_WEIGHT,
                    &mut [for_user(), loopback()],
                )
            });
    }
    // SAFETY: descriptor was allocated by the conversion above and is freed once.
    unsafe { LocalFree(descriptor as _) };
    result
}

fn enable_debug_privilege() -> Result<(), String> {
    let mut luid = LUID {
        LowPart: 0,
        HighPart: 0,
    };
    // SAFETY: the wide name outlives the call and luid is a valid out-pointer.
    if unsafe { LookupPrivilegeValueW(null(), wide("SeDebugPrivilege").as_ptr(), &mut luid) } == 0 {
        return Err("cannot look up SeDebugPrivilege".to_owned());
    }
    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    let mut token: HANDLE = null_mut();
    // SAFETY: token is a valid out-pointer and GetCurrentProcess returns a pseudo handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES, &mut token) } == 0 {
        return Err("cannot open the process token".to_owned());
    }
    // SAFETY: token is open and privileges outlives the call; the previous-state pointers are null.
    let enabled = unsafe {
        AdjustTokenPrivileges(token, 0, &privileges, 0, null_mut(), null_mut()) != 0
            && GetLastError() != ERROR_NOT_ALL_ASSIGNED
    };
    // SAFETY: token is open and closed once.
    unsafe { CloseHandle(token) };
    if enabled {
        Ok(())
    } else {
        Err("cannot enable SeDebugPrivilege to reach the DNS Client service".to_owned())
    }
}

fn dns_client_pid() -> Result<u32, String> {
    // SAFETY: null machine and database name are allowed; a null result is checked.
    let manager = unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT) };
    if manager.is_null() {
        return Err("cannot open the service manager".to_owned());
    }
    // SAFETY: manager is open and the wide name outlives the call; a null result is checked.
    let service = unsafe { OpenServiceW(manager, wide("Dnscache").as_ptr(), SERVICE_QUERY_STATUS) };
    let mut status = [0u32; 9];
    let mut needed = 0u32;
    let ok = !service.is_null()
        // SAFETY: service is non-null and status is a buffer of the size passed.
        && unsafe {
            QueryServiceStatusEx(
                service,
                SC_STATUS_PROCESS_INFO,
                status.as_mut_ptr() as *mut u8,
                std::mem::size_of_val(&status) as u32,
                &mut needed,
            )
        } != 0;
    // SAFETY: service and manager are open handles, each closed once.
    unsafe {
        if !service.is_null() {
            CloseServiceHandle(service);
        }
        CloseServiceHandle(manager);
    }
    match status.get(7) {
        Some(&pid) if ok && pid != 0 => Ok(pid),
        _ => Err("cannot find the process of the Dnscache service".to_owned()),
    }
}

fn handle_entries() -> Result<Vec<HandleEntry>, String> {
    let mut buffer = vec![0u64; 1 << 17];
    loop {
        let len = (buffer.len() * 8) as u32;
        // SAFETY: buffer is valid for len bytes and the query accepts a null return length.
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_EXTENDED_HANDLE_INFORMATION,
                buffer.as_mut_ptr() as *mut c_void,
                len,
                null_mut(),
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status < 0 {
            return Err(format!(
                "cannot list the system handles (NTSTATUS {status:#x})"
            ));
        }
        let count = buffer.first().copied().unwrap_or(0) as usize;
        // SAFETY: the query filled buffer, so the entries start two words in.
        let first = unsafe { buffer.as_ptr().add(2) } as *const HandleEntry;
        return Ok((0..count)
            // SAFETY: at is below count, so the read stays inside the entries the query wrote.
            .map(|at| unsafe { std::ptr::read(first.add(at)) })
            .collect());
    }
}

/// The UNICODE_STRING that NtQueryObject puts at the start of both the name and the type.
fn object_text(handle: HANDLE, class: u32) -> Option<String> {
    let mut buffer = vec![0u64; 512];
    // SAFETY: buffer is valid for the byte length passed and handle is an open handle.
    let status = unsafe {
        NtQueryObject(
            handle,
            class,
            buffer.as_mut_ptr() as *mut c_void,
            (buffer.len() * 8) as u32,
            null_mut(),
        )
    };
    if status < 0 {
        return None;
    }
    // SAFETY: the query wrote a UNICODE_STRING at the start of buffer, which is 8-byte aligned.
    let text = unsafe { &*(buffer.as_ptr() as *const UnicodeString) };
    if text.buffer.is_null() {
        return Some(String::new());
    }
    // SAFETY: text.buffer points into buffer and holds length / 2 u16 values.
    let chars = unsafe { std::slice::from_raw_parts(text.buffer, text.length as usize / 2) };
    Some(String::from_utf16_lossy(chars))
}

/// Duplicates the Dnscache process's handle to its `\RPC Control\DNSResolver` ALPC port.
fn find_dns_port() -> Result<HANDLE, String> {
    let pid = dns_client_pid()?;
    let entries = handle_entries()?;
    // SAFETY: OpenProcess takes plain integers and a null result is checked.
    let process = unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, pid) };
    if process.is_null() {
        return Err(format!(
            "cannot open the Dnscache process {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut seen = Vec::new();
    let mut found: HANDLE = null_mut();
    for entry in entries.iter().filter(|entry| entry.pid == pid as usize) {
        let mut copy: HANDLE = null_mut();
        // SAFETY: process and the source handle are valid and copy is a valid out-pointer.
        let duplicated = unsafe {
            DuplicateHandle(
                process,
                entry.handle as HANDLE,
                GetCurrentProcess(),
                &mut copy,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if duplicated == 0 {
            continue;
        }
        // Only ALPC ports are named here: a name query on some pipes never returns.
        if object_text(copy, OBJECT_TYPE_INFORMATION).as_deref() == Some("ALPC Port") {
            let name = object_text(copy, OBJECT_NAME_INFORMATION).unwrap_or_default();
            if name.to_ascii_lowercase().ends_with("\\dnsresolver") {
                found = copy;
                break;
            }
            seen.push(name);
        }
        // SAFETY: copy is a handle duplicated above and closed once.
        unsafe { CloseHandle(copy) };
    }
    // SAFETY: process is open and closed once.
    unsafe { CloseHandle(process) };
    if found.is_null() {
        return Err(format!(
            "no DNSResolver ALPC port in the Dnscache process {pid}; its ports: {seen:?}"
        ));
    }
    Ok(found)
}

/// Adds (or removes) the account's entries on an ALPC port, under a machine-wide lock so
/// concurrent runs do not lose each other's entries.
fn set_port_access(port: HANDLE, sid: *mut c_void, mode: i32) -> Result<(), String> {
    // SAFETY: the wide name outlives the call and a null mutex is handled below.
    let lock = unsafe { CreateMutexW(null(), 0, wide(DNS_PORT_LOCK).as_ptr()) };
    if lock.is_null() {
        return Err(format!(
            "cannot create the DNSResolver ACL lock: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: lock is the valid mutex handle created above.
    unsafe { WaitForSingleObject(lock, INFINITE) };
    let mut old: *mut ACL = null_mut();
    let mut descriptor: *mut c_void = null_mut();
    let mut merged: *mut ACL = null_mut();
    let entry = trustee_entry(sid, PORT_ALL_ACCESS, mode);
    let result = check(
        // SAFETY: port is an open handle; old and descriptor are valid out-pointers.
        unsafe {
            GetSecurityInfo(
                port,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old,
                null_mut(),
                &mut descriptor,
            )
        },
        "reading the ACL of the DNSResolver port",
    )
    .and_then(|()| {
        // A deny entry on its own would turn every caller away, not just this account.
        if old.is_null() {
            Err("the DNSResolver port has no ACL to add the account's deny entry to".to_owned())
        } else {
            Ok(())
        }
    })
    .and_then(|()| {
        check(
            // SAFETY: entry is valid, old is non-null and merged is a valid out-pointer.
            unsafe { SetEntriesInAclW(1, &entry, old, &mut merged) },
            "building the ACL of the DNSResolver port",
        )
    })
    .and_then(|()| {
        check(
            // SAFETY: port is an open handle and merged is the ACL built above.
            unsafe {
                SetSecurityInfo(
                    port,
                    SE_KERNEL_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    merged,
                    null(),
                )
            },
            "setting the ACL of the DNSResolver port (the duplicated handle needs WRITE_DAC)",
        )
    });
    // SAFETY: merged and descriptor are freed once; lock is owned, released and closed once.
    unsafe {
        LocalFree(merged as _);
        LocalFree(descriptor as _);
        ReleaseMutex(lock);
        CloseHandle(lock);
    }
    result
}

/// Dnscache resolves under its own account, which the network filters do not cover, so the
/// account is turned away at the service's RPC port. dnsapi then queries in process, where
/// the filters apply.
fn deny_dns_client(session: &mut Session) -> Result<(), String> {
    enable_debug_privilege()?;
    let pid = dns_client_pid()?;
    let port = find_dns_port()?;
    // SAFETY: port is the open handle from find_dns_port, closed once on this failure path.
    set_port_access(port, session.sid(), DENY_ACCESS).inspect_err(|_| unsafe {
        CloseHandle(port);
    })?;
    session.dns_port = port;
    session.dns_pid = pid;
    Ok(())
}

/// A restarted Dnscache recreates its port with the default ACL, which drops the deny entry.
fn check_dns_client(session: &Session) -> Result<(), String> {
    if session.dns_pid != 0 && dns_client_pid()? != session.dns_pid {
        return Err(
            "the Dnscache service restarted, so the run account's DNS deny is gone".to_owned(),
        );
    }
    Ok(())
}

fn spawn(config: &Config, session: &Session, password: &str) -> Result<i32, String> {
    // SAFETY: STARTUPINFOW is plain C data for which all-zero is valid.
    let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    startup.dwFlags = STARTF_USESTDHANDLES;
    // SAFETY: GetStdHandle takes plain constants and has no preconditions.
    unsafe {
        startup.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
        startup.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
        startup.hStdError = GetStdHandle(STD_ERROR_HANDLE);
    }
    let parent: Vec<(String, String)> = std::env::vars_os()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let (environment, _) = environment(&parent, &user_defaults()?);
    let cwd = std::env::current_dir().map_err(|err| format!("cannot read the directory: {err}"))?;
    let cwd = wide(&cwd.to_string_lossy());
    check_dns_client(session)?;
    let mut line = wide(&logon_command_line(&config.command)?);
    // SAFETY: PROCESS_INFORMATION is plain C data for which all-zero is valid.
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let mut attempts = 0;
    let created = loop {
        // SAFETY: the strings are NUL-terminated and outlive the call; line is writable.
        let created = unsafe {
            CreateProcessWithLogonW(
                session.user.as_ptr(),
                wide(".").as_ptr(),
                wide(password).as_ptr(),
                LOGON_WITH_PROFILE,
                null(),
                line.as_mut_ptr(),
                CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
                environment.as_ptr() as *const c_void,
                cwd.as_ptr(),
                &startup,
                &mut info,
            )
        };
        // The secondary logon service answers ERROR_SERVICE_ALREADY_RUNNING while another logon is in flight.
        // SAFETY: GetLastError has no preconditions.
        let busy = unsafe { GetLastError() } == 1056;
        attempts += 1;
        if created != 0 || !busy || attempts >= 20 {
            break created;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    if created == 0 {
        return Err(format!(
            "cannot start {} as the run account: {}",
            config.command.first().map_or("", String::as_str),
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: session.job and info.hProcess are valid handles.
    if unsafe { AssignProcessToJobObject(session.job, info.hProcess) } == 0 {
        // SAFETY: info.hThread is the live handle from CreateProcessWithLogonW, closed once.
        unsafe { CloseHandle(info.hThread) };
        // SAFETY: info.hProcess is the live handle from CreateProcessWithLogonW, closed once.
        unsafe { CloseHandle(info.hProcess) };
        return Err("cannot put the command in the job object".to_owned());
    }
    let mut code = 1u32;
    // SAFETY: info holds the live handles from CreateProcessWithLogonW; hThread is closed once.
    unsafe {
        ResumeThread(info.hThread);
        CloseHandle(info.hThread);
    }
    let mut outcome = Ok(());
    // SAFETY: info.hProcess is a live process handle.
    while unsafe { WaitForSingleObject(info.hProcess, 1000) } == WAIT_TIMEOUT {
        if let Err(err) = check_dns_client(session) {
            // SAFETY: session.job is a valid job handle.
            unsafe { TerminateJobObject(session.job, 1) };
            outcome = Err(err);
            break;
        }
    }
    // SAFETY: info.hProcess is live, code is a valid out-pointer and the handle is closed once.
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
    }
    outcome?;
    Ok(code as i32)
}

pub fn run(config: &Config) -> Result<i32, String> {
    let mut session = Session {
        user: wide(&user_name(&config.name)),
        created: false,
        sid: Vec::new(),
        sid_text: String::new(),
        job: null_mut(),
        acl_paths: Vec::new(),
        engine: null_mut(),
        dns_port: null_mut(),
        dns_pid: 0,
    };
    let password = random_password()?;
    create_user(&mut session, &password)?;
    look_up_sid(&mut session)?;
    if !config.net {
        block_network(&mut session)?;
        deny_dns_client(&mut session)?;
    }
    if let Some(group) = &config.group {
        // Before the logon, so the groups are in the account's token.
        for group in [group.as_str(), HOST_GROUP] {
            add_member(
                &session.user,
                group,
                &format!("adding the account to {group}"),
            )?;
        }
    } else {
        for (path, mode) in acl_entries(config) {
            set_access(path, session.sid(), mode)?;
            session.acl_paths.push(path.clone());
        }
    }
    session.job = kill_on_close_job()?;
    spawn(config, &session, &password)
}

fn acl_entries(config: &Config) -> impl Iterator<Item = (&String, i32)> {
    config
        .grants
        .iter()
        .map(|path| (path, GRANT_ACCESS))
        .chain(config.denies.iter().map(|path| (path, DENY_ACCESS)))
}

/// Creates the local group and gives it the grants and denies, inherited by everything below.
/// Undoes all of it if any step fails.
pub fn setup_group(config: &Config) -> Result<i32, String> {
    let group = config.group.as_deref().ok_or("no group to set up")?;
    remove_stale_groups(config, group);
    let name = create_group(group)?;
    let applied = account_sid(&name).and_then(|sid| {
        let mut first = Ok(());
        for (path, mode, millis, result) in in_waves(config, &sid, set_access) {
            match result {
                Ok(()) => {
                    let kind = if mode == DENY_ACCESS { "deny" } else { "grant" };
                    println!("acl {kind} {path}: {millis}ms");
                }
                Err(err) if first.is_ok() => first = Err(err),
                Err(_) => {}
            }
        }
        first
    });
    if let Err(err) = applied {
        let _ = teardown_group(config);
        return Err(err);
    }
    Ok(0)
}

/// Creates the local group unless it exists, and returns its name as UTF-16.
fn create_group(group: &str) -> Result<Vec<u16>, String> {
    let name = wide(group);
    let mut info_name = name.clone();
    let info = LOCALGROUP_INFO_0 {
        lgrpi0_name: info_name.as_mut_ptr(),
    };
    // SAFETY: info is a valid LOCALGROUP_INFO_0 whose name outlives the call.
    let code = unsafe { NetLocalGroupAdd(null(), 0, &info as *const _ as *const u8, null_mut()) };
    if code == ERROR_ACCESS_DENIED {
        return Err(
            "NetLocalGroupAdd failed with error 5: the user backend must run elevated (as administrator)"
                .to_owned(),
        );
    }
    if code != ERROR_ALIAS_EXISTS {
        check(code, &format!("creating the local group {group}"))?;
    }
    Ok(name)
}

/// Gives `dxh-host` an inherited deny on every path that does not have it yet. A path counts as
/// done only when the host deny list names it and its ACL still holds the group's own entry, and
/// it joins the list only after its whole tree is written, so an interrupted walk is redone.
pub fn ensure_host_denies(config: &Config) -> Result<i32, String> {
    with_acl_lock(HOST_DENIES_LOCK, || {
        let sid = account_sid(&create_group(HOST_GROUP)?)?;
        let list = host_deny_list();
        let mut listed = read_host_denies(&list);
        let mut pending = Vec::new();
        for path in &config.denies {
            let key = path_key(path);
            if listed.contains(&key) && has_explicit_deny(path, &sid)? {
                println!("acl deny {path}: present");
            } else if !pending.contains(path) {
                pending.push(path.clone());
            }
        }
        let pending = host_config(pending);
        let mut first = Ok(0);
        for (path, _, millis, result) in in_waves(&pending, &sid, set_access) {
            match result {
                Ok(()) => {
                    println!("acl deny {path}: {millis}ms");
                    let key = path_key(path);
                    if !listed.contains(&key) {
                        listed.push(key);
                    }
                    write_host_denies(&list, &listed)?;
                }
                Err(err) if first.is_ok() => first = Err(err),
                Err(_) => {}
            }
        }
        first
    })
}

/// Takes the `dxh-host` entries off the paths and the paths off the host deny list. The group
/// stays, since runs on other trees may still use it.
pub fn remove_host_denies(config: &Config) -> Result<i32, String> {
    with_acl_lock(HOST_DENIES_LOCK, || {
        let sid = account_sid(&wide(HOST_GROUP))?;
        let list = host_deny_list();
        let mut listed = read_host_denies(&list);
        let mut failed = Vec::new();
        for (path, _, millis, result) in in_waves(config, &sid, revoke) {
            match result {
                Ok(()) => {
                    println!("acl revoke {path}: {millis}ms");
                    let key = path_key(path);
                    listed.retain(|have| *have != key);
                }
                Err(err) => failed.push(err),
            }
        }
        write_host_denies(&list, &listed)?;
        if failed.is_empty() {
            Ok(0)
        } else {
            Err(failed.join("; "))
        }
    })
}

fn host_config(denies: Vec<String>) -> Config {
    Config {
        mode: Mode::EnsureHostDenies,
        name: String::new(),
        backend: crate::args::Backend::User,
        net: false,
        group: Some(HOST_GROUP.to_owned()),
        grants: Vec::new(),
        denies,
        command: Vec::new(),
    }
}

fn host_deny_list() -> std::path::PathBuf {
    let data = std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into());
    std::path::Path::new(&data)
        .join("dx-hermetic")
        .join("host-denies.txt")
}

fn read_host_denies(list: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(list)
        .unwrap_or_default()
        .lines()
        .map(path_key)
        .filter(|key| !key.is_empty())
        .collect()
}

fn write_host_denies(list: &std::path::Path, keys: &[String]) -> Result<(), String> {
    let text: String = keys.iter().map(|key| format!("{key}\n")).collect();
    let partial = list.with_extension("tmp");
    list.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&partial, text))
        .and_then(|()| std::fs::rename(&partial, list))
        .map_err(|err| format!("cannot write {}: {err}", list.display()))
}

/// Removes the group's entries from every path, then the group itself.
pub fn teardown_group(config: &Config) -> Result<i32, String> {
    let group = config.group.as_deref().ok_or("no group to tear down")?;
    let name = wide(group);
    let sid = account_sid(&name)?;
    let mut failed = Vec::new();
    for (path, _, millis, result) in in_waves(config, &sid, revoke) {
        match result {
            Ok(()) => println!("acl revoke {path}: {millis}ms"),
            Err(err) => failed.push(err),
        }
    }
    // SAFETY: name is a NUL-terminated wide string and null selects the local server.
    let code = unsafe { NetLocalGroupDel(null(), name.as_ptr()) };
    if let Err(err) = check(code, &format!("deleting the local group {group}")) {
        failed.push(err);
    }
    if failed.is_empty() {
        Ok(0)
    } else {
        Err(failed.join("; "))
    }
}

/// Removes the entries for `sid` from every grant and deny path, returning the errors.
fn remove_group_entries(config: &Config, sid: &[u8]) -> Vec<String> {
    in_waves(config, sid, revoke)
        .into_iter()
        .filter_map(|(_, _, _, result)| result.err())
        .collect()
}

fn revoke(path: &str, sid: *mut c_void, _mode: i32) -> Result<(), String> {
    remove_access(path, sid)
}

type AclStep = fn(&str, *mut c_void, i32) -> Result<(), String>;

/// Runs `step` on every grant and deny path for `sid`, all paths of one wave at the same time,
/// and returns each path's mode, milliseconds and result in entry order.
fn in_waves<'a>(
    config: &'a Config,
    sid: &[u8],
    step: AclStep,
) -> Vec<(&'a String, i32, u128, Result<(), String>)> {
    let entries: Vec<(&String, i32)> = acl_entries(config).collect();
    let paths: Vec<&str> = entries.iter().map(|(path, _)| path.as_str()).collect();
    let mut done: Vec<Option<(u128, Result<(), String>)>> = entries.iter().map(|_| None).collect();
    for wave in acl_waves(&paths) {
        std::thread::scope(|scope| {
            let threads: Vec<_> = wave
                .iter()
                .filter_map(|&at| {
                    let &(path, mode) = entries.get(at)?;
                    // Each thread has its own copy, as raw pointers cannot cross threads.
                    let mut sid = sid.to_vec();
                    let thread = scope.spawn(move || {
                        let start = Instant::now();
                        let result = step(path, sid.as_mut_ptr() as *mut c_void, mode);
                        (start.elapsed().as_millis(), result)
                    });
                    Some((at, path, thread))
                })
                .collect();
            for (at, path, thread) in threads {
                let outcome = thread
                    .join()
                    .unwrap_or_else(|_| (0, Err(format!("changing the ACL of {path} panicked"))));
                if let Some(slot) = done.get_mut(at) {
                    *slot = Some(outcome);
                }
            }
        });
    }
    entries
        .into_iter()
        .zip(done)
        .map(|((path, mode), outcome)| {
            let (millis, result) = outcome.expect("every path is in a wave");
            (path, mode, millis, result)
        })
        .collect()
}

/// Deletes the groups of runs that were killed before their teardown. Only warns on failure, and
/// keeps a group whose entries could not all be removed so a later run can retry.
fn remove_stale_groups(config: &Config, own: &str) {
    let names = match local_groups() {
        Ok(names) => names,
        Err(err) => {
            eprintln!("appcontainer_run: warning: {err}");
            return;
        }
    };
    for group in stale_groups(&names, own, process_alive) {
        let name = wide(&group);
        let sid = match account_sid(&name) {
            Ok(sid) => sid,
            Err(err) => {
                eprintln!("appcontainer_run: warning: {err}");
                continue;
            }
        };
        let failed = remove_group_entries(config, &sid);
        if !failed.is_empty() {
            eprintln!(
                "appcontainer_run: warning: keeping the stale group {group}: {}",
                failed.join("; ")
            );
            continue;
        }
        // SAFETY: name is a NUL-terminated wide string and null selects the local server.
        let code = unsafe { NetLocalGroupDel(null(), name.as_ptr()) };
        if let Err(err) = check(code, &format!("deleting the stale local group {group}")) {
            eprintln!("appcontainer_run: warning: {err}");
        }
    }
}

fn local_groups() -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let mut resume: usize = 0;
    loop {
        let mut buffer: *mut u8 = null_mut();
        let (mut read, mut total) = (0u32, 0u32);
        // SAFETY: buffer, read, total and resume are valid out-pointers.
        let code = unsafe {
            NetLocalGroupEnum(
                null(),
                0,
                &mut buffer,
                MAX_PREFERRED_LENGTH,
                &mut read,
                &mut total,
                &mut resume,
            )
        };
        if !buffer.is_null() {
            if code == 0 || code == ERROR_MORE_DATA {
                // SAFETY: buffer holds read LOCALGROUP_INFO_0 entries written by NetLocalGroupEnum.
                let entries = unsafe {
                    std::slice::from_raw_parts(buffer as *const LOCALGROUP_INFO_0, read as usize)
                };
                for entry in entries {
                    // SAFETY: each lgrpi0_name is a NUL-terminated string inside the enum buffer.
                    let name = unsafe { wide_text(entry.lgrpi0_name) };
                    names.push(name);
                }
            }
            // SAFETY: buffer was allocated by NetLocalGroupEnum and is freed once.
            unsafe { NetApiBufferFree(buffer as *const c_void) };
        }
        if code != ERROR_MORE_DATA {
            check(code, "listing the local groups")?;
            return Ok(names);
        }
    }
}

/// Reads a NUL-terminated UTF-16 string.
unsafe fn wide_text(text: *const u16) -> String {
    let mut len = 0;
    // SAFETY: the caller guarantees text is NUL-terminated, so len stays in bounds.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: text is valid for len u16 values, as counted above.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) })
}

/// Whether `pid` may still run. Only a pid Windows does not know counts as gone, so a process
/// we may not open is treated as alive.
fn process_alive(pid: u32) -> bool {
    // SAFETY: OpenProcess takes plain integers and a null result is checked.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        // SAFETY: GetLastError has no preconditions.
        return unsafe { GetLastError() } != ERROR_INVALID_PARAMETER;
    }
    // SAFETY: process is the open handle from OpenProcess.
    let running = unsafe { WaitForSingleObject(process, 0) } == WAIT_TIMEOUT;
    // SAFETY: process is open and closed once.
    unsafe { CloseHandle(process) };
    running
}

/// Reads the DACL of one path. The ACL points into the descriptor, which the caller frees.
fn read_acl(path: &str) -> Result<(*mut ACL, *mut c_void), String> {
    let mut acl: *mut ACL = null_mut();
    let mut descriptor: *mut c_void = null_mut();
    check(
        // SAFETY: wide(path) outlives the call and acl and descriptor are valid out-pointers.
        unsafe {
            GetNamedSecurityInfoW(
                wide(path).as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut acl,
                null_mut(),
                &mut descriptor,
            )
        },
        &format!("reading the ACL of {path}"),
    )?;
    Ok((acl, descriptor))
}

/// The index and type of every explicit allow and deny entry for `sid`, last first.
fn own_entries(acl: *mut ACL, sid: *const c_void) -> Vec<(u32, u8)> {
    let mut found = Vec::new();
    if acl.is_null() {
        return found;
    }
    // SAFETY: acl was checked non-null and points to a valid ACL header.
    for index in (0..unsafe { (*acl).AceCount } as u32).rev() {
        let mut ace: *mut c_void = null_mut();
        // SAFETY: acl is valid, index is below AceCount and ace is a valid out-pointer.
        if unsafe { GetAce(acl, index, &mut ace) } == 0 {
            continue;
        }
        let ace = ace as *mut ACCESS_ALLOWED_ACE;
        // SAFETY: GetAce returned a valid ACE pointer, which starts with an ACE header.
        let header = unsafe { (*ace).Header };
        if matches!(
            header.AceType,
            ACCESS_ALLOWED_ACE_TYPE | ACCESS_DENIED_ACE_TYPE
        ) && u32::from(header.AceFlags) & INHERITED_ACE == 0
            // SAFETY: ace has a SID at SidStart and sid points to a valid SID.
            && unsafe { EqualSid(&raw mut (*ace).SidStart as *mut c_void, sid as *mut c_void) } != 0
        {
            found.push((index, header.AceType));
        }
    }
    found
}

fn has_explicit_deny(path: &str, sid: &[u8]) -> Result<bool, String> {
    let (acl, descriptor) = read_acl(path)?;
    let denied = own_entries(acl, sid.as_ptr() as *const c_void)
        .iter()
        .any(|(_, kind)| *kind == ACCESS_DENIED_ACE_TYPE);
    // SAFETY: descriptor was allocated by read_acl and is freed once.
    unsafe { LocalFree(descriptor as _) };
    Ok(denied)
}

/// Deletes every explicit allow and deny entry for `sid` on one path, under the path's lock.
/// `REVOKE_ACCESS` keeps the deny entries, so it cannot undo a `DENY_ACCESS`.
fn remove_access(path: &str, sid: *mut c_void) -> Result<(), String> {
    with_acl_lock(&path_lock_name(path), || remove_access_locked(path, sid))
}

fn remove_access_locked(path: &str, sid: *mut c_void) -> Result<(), String> {
    let wpath = wide(path);
    let (acl, descriptor) = read_acl(path)?;
    let mut removed = false;
    for (index, _) in own_entries(acl, sid) {
        // SAFETY: acl is valid and index came from own_entries on it.
        if unsafe { DeleteAce(acl, index) } != 0 {
            removed = true;
        }
    }
    let result = if removed {
        check(
            // SAFETY: wpath outlives the call and acl is the DACL read from the same path.
            unsafe {
                SetNamedSecurityInfoW(
                    wpath.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    acl,
                    null_mut(),
                )
            },
            &format!("setting the ACL of {path}"),
        )
    } else {
        Ok(())
    };
    // SAFETY: descriptor was allocated by read_acl and is freed once.
    unsafe { LocalFree(descriptor as _) };
    result
}
