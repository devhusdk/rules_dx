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
use dx_os_sys::job_object::{JobObject, Limit};
use dx_os_sys::{nt, win32};
use std::ffi::c_void;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_INVALID_PARAMETER, ERROR_MORE_DATA,
    ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, LUID,
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
    SetEntriesInAclW, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, AdjustTokenPrivileges, DACL_SECURITY_INFORMATION, DeleteAce, EqualSid,
    GetAce, GetSecurityDescriptorLength, INHERITED_ACE, LUID_AND_ATTRIBUTES, LookupAccountNameW,
    LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessWithLogonW, GetCurrentProcess, LOGON_WITH_PROFILE, PROCESS_INFORMATION,
    STARTUPINFOW,
};
use windows_sys::Win32::UI::Shell::DeleteProfileW;
use windows_sys::core::GUID;

use crate::args::{
    Config, HOST_GROUP, Mode, acl_waves, environment, logon_command_line, password, path_key,
    stale_groups, user_name,
};
use crate::sys::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, DENY_ACCESS, GRANT_ACCESS, LocalMem, NamedLock,
    REVOKE_ACCESS, STARTF_USESTDHANDLES, check, path_lock_name, process_token, set_access,
    start_in_job, std_handles, trustee_entry, user_defaults, wait_exit, wide, wide_text,
    with_acl_lock, write_acl, write_acl_here,
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
const SC_MANAGER_CONNECT: u32 = 1;
const SERVICE_QUERY_STATUS: u32 = 4;
const SC_STATUS_PROCESS_INFO: i32 = 0;
const DNS_PORT_LOCK: &str = "Global\\dx_hermetic_dnsresolver_acl";
const HOST_DENIES_LOCK: &str = "Global\\dx-hermetic-host-denies";
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;

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

/// An open service manager or service handle, closed on drop.
struct ServiceHandle(HANDLE);

impl ServiceHandle {
    fn new(raw: HANDLE, what: &str) -> Result<Self, String> {
        if raw.is_null() {
            return Err(format!(
                "cannot open {what}: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self(raw))
    }
}

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        // SAFETY: the handle is open and closed only here.
        unsafe { CloseServiceHandle(self.0) };
    }
}

/// An open filtering engine session. Closing it removes the session's dynamic filters.
struct Engine(HANDLE);

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: the engine handle is open and closed only here.
        unsafe { FwpmEngineClose0(self.0) };
    }
}

/// A buffer the NetApi calls allocated, freed on drop.
struct NetBuffer(*mut u8);

impl Drop for NetBuffer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the buffer came from a NetApi call and is freed only here.
            unsafe { NetApiBufferFree(self.0 as *const c_void) };
        }
    }
}

/// Everything a run creates. Dropping it undoes whatever exists, in a fixed order.
struct Session {
    user: Vec<u16>,
    created: bool,
    sid: Vec<u8>,
    sid_text: String,
    job: Option<JobObject>,
    acl_paths: Vec<String>,
    engine: Option<Engine>,
    dns_port: Option<OwnedHandle>,
    dns_pid: u32,
}

impl Session {
    fn sid(&self) -> *mut c_void {
        self.sid.as_ptr() as *mut c_void
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            end_job(&job);
        }
        for path in &self.acl_paths {
            let _ = remove_access(path, self.sid());
        }
        if let Some(port) = self.dns_port.take()
            && let Err(err) = set_port_access(port.as_handle(), self.sid(), REVOKE_ACCESS)
        {
            eprintln!("appcontainer_run: {err}");
        }
        if self.created {
            if !self.sid_text.is_empty() {
                delete_profile(&self.sid_text);
            }
            // SAFETY: user is a NUL-terminated wide name and null selects the local server.
            unsafe { NetUserDel(null(), self.user.as_ptr()) };
        }
        self.engine = None;
    }
}

/// Kills what is left in the job and waits until it is empty, so the profile is no longer in use.
fn end_job(job: &JobObject) {
    let _ = job.terminate(1);
    for _ in 0..100 {
        if job.active_processes().is_ok_and(|active| active > 0) {
            std::thread::sleep(Duration::from_millis(100));
        } else {
            break;
        }
    }
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
    let mut text = LocalMem::<u16>::null();
    // SAFETY: session.sid() is a valid SID and text.out() is a valid out-pointer.
    if unsafe { ConvertSidToStringSidW(session.sid(), text.out()) } == 0 {
        return Err(format!(
            "cannot print the account SID: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: text is the NUL-terminated string ConvertSidToStringSidW returned.
    session.sid_text = unsafe { wide_text(text.get()) };
    Ok(())
}

fn add_filter(
    engine: &Engine,
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
        unsafe { FwpmFilterAdd0(engine.0, &filter, null_mut(), &mut id) },
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
    let mut raw: HANDLE = null_mut();
    check(
        // SAFETY: wfp is valid, label outlives the call and raw is a valid out-pointer.
        unsafe { FwpmEngineOpen0(null(), RPC_C_AUTHN_DEFAULT, null(), &wfp, &mut raw) },
        "opening the filtering engine",
    )?;
    let engine = session.engine.insert(Engine(raw));

    let sddl = wide(&format!("O:LSD:(A;;CC;;;{})", session.sid_text));
    let mut descriptor = LocalMem::<c_void>::null();
    // SAFETY: sddl is NUL-terminated and descriptor.out() is a valid out-pointer.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            descriptor.out(),
            null_mut(),
        )
    } == 0
    {
        return Err("cannot build the account match for the network filters".to_owned());
    }
    let mut blob = FWP_BYTE_BLOB {
        // SAFETY: descriptor was just returned by the conversion above.
        size: unsafe { GetSecurityDescriptorLength(descriptor.get()) },
        data: descriptor.get() as *mut u8,
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
                engine,
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
                    engine,
                    layer,
                    FWP_ACTION_BLOCK,
                    BLOCK_WEIGHT,
                    &mut [for_user()],
                )
            })
            .and_then(|()| {
                add_filter(
                    engine,
                    layer,
                    FWP_ACTION_PERMIT,
                    PERMIT_WEIGHT,
                    &mut [for_user(), loopback()],
                )
            });
    }
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
    let token = process_token(TOKEN_ADJUST_PRIVILEGES)?;
    // SAFETY: token is open and privileges outlives the call; the previous-state pointers are null.
    let adjusted = unsafe {
        AdjustTokenPrivileges(
            token.as_raw_handle(),
            0,
            &privileges,
            0,
            null_mut(),
            null_mut(),
        )
    };
    // SAFETY: GetLastError has no preconditions.
    let enabled = adjusted != 0 && unsafe { GetLastError() } != ERROR_NOT_ALL_ASSIGNED;
    if enabled {
        Ok(())
    } else {
        Err("cannot enable SeDebugPrivilege to reach the DNS Client service".to_owned())
    }
}

fn dns_client_pid() -> Result<u32, String> {
    // SAFETY: null machine and database name are allowed; a null result is checked.
    let raw = unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT) };
    let manager = ServiceHandle::new(raw, "the service manager")?;
    let name = wide("Dnscache");
    // SAFETY: manager is open and name outlives the call; a null result is checked.
    let raw = unsafe { OpenServiceW(manager.0, name.as_ptr(), SERVICE_QUERY_STATUS) };
    let service = ServiceHandle::new(raw, "the Dnscache service")?;
    let mut status = [0u32; 9];
    let mut needed = 0u32;
    // SAFETY: service is open and status is a buffer of the size passed.
    let ok = unsafe {
        QueryServiceStatusEx(
            service.0,
            SC_STATUS_PROCESS_INFO,
            status.as_mut_ptr() as *mut u8,
            std::mem::size_of_val(&status) as u32,
            &mut needed,
        )
    } != 0;
    match status.get(7) {
        Some(&pid) if ok && pid != 0 => Ok(pid),
        _ => Err("cannot find the process of the Dnscache service".to_owned()),
    }
}

/// Duplicates the Dnscache process's handle to its `\RPC Control\DNSResolver` ALPC port.
fn find_dns_port() -> Result<OwnedHandle, String> {
    let pid = dns_client_pid()?;
    let entries =
        nt::system_handles().map_err(|err| format!("cannot list the system handles: {err}"))?;
    let process = win32::open(pid, win32::Access::Duplicate)
        .map_err(|err| format!("cannot open the Dnscache process {pid}: {err}"))?;
    // SAFETY: GetCurrentProcess takes no arguments and returns a pseudo handle.
    let current = unsafe { GetCurrentProcess() };
    let mut seen = Vec::new();
    for entry in entries.iter().filter(|entry| entry.pid == pid as usize) {
        let mut raw: HANDLE = null_mut();
        // SAFETY: process has PROCESS_DUP_HANDLE access, the source handle is a value in it
        // and raw is a valid out-pointer.
        let duplicated = unsafe {
            DuplicateHandle(
                process.as_raw_handle(),
                entry.handle as HANDLE,
                current,
                &mut raw,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if duplicated == 0 {
            continue;
        }
        // SAFETY: DuplicateHandle succeeded, so raw is a new handle owned by the caller.
        let copy = unsafe { OwnedHandle::from_raw_handle(raw) };
        // Only ALPC ports are named here: a name query on some pipes never returns.
        if nt::object_type_name(copy.as_handle()).ok().as_deref() == Some("ALPC Port") {
            let name = nt::object_name(copy.as_handle()).unwrap_or_default();
            if name.to_ascii_lowercase().ends_with("\\dnsresolver") {
                return Ok(copy);
            }
            seen.push(name);
        }
    }
    Err(format!(
        "no DNSResolver ALPC port in the Dnscache process {pid}; its ports: {seen:?}"
    ))
}

/// Adds (or removes) the account's entries on an ALPC port, under a machine-wide lock so
/// concurrent runs do not lose each other's entries.
fn set_port_access(port: BorrowedHandle<'_>, sid: *mut c_void, mode: i32) -> Result<(), String> {
    let _lock = NamedLock::acquire(DNS_PORT_LOCK)?;
    let port = port.as_raw_handle();
    let mut old: *mut ACL = null_mut();
    let mut descriptor = LocalMem::<c_void>::null();
    let mut merged = LocalMem::<ACL>::null();
    let entry = trustee_entry(sid, PORT_ALL_ACCESS, mode);

    check(
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
                descriptor.out(),
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
            unsafe { SetEntriesInAclW(1, &entry, old, merged.out()) },
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
                    merged.get(),
                    null(),
                )
            },
            "setting the ACL of the DNSResolver port (the duplicated handle needs WRITE_DAC)",
        )
    })
}

/// Dnscache resolves under its own account, which the network filters do not cover, so the
/// account is turned away at the service's RPC port. dnsapi then queries in process, where
/// the filters apply.
fn deny_dns_client(session: &mut Session) -> Result<(), String> {
    enable_debug_privilege()?;
    let pid = dns_client_pid()?;
    let port = find_dns_port()?;
    set_port_access(port.as_handle(), session.sid(), DENY_ACCESS)?;
    session.dns_port = Some(port);
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
    [startup.hStdInput, startup.hStdOutput, startup.hStdError] = std_handles();
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
    let domain = wide(".");
    let password = wide(password);
    // SAFETY: PROCESS_INFORMATION is plain C data for which all-zero is valid.
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let mut attempts = 0;
    let created = loop {
        // SAFETY: the strings are NUL-terminated and outlive the call; line is writable.
        let created = unsafe {
            CreateProcessWithLogonW(
                session.user.as_ptr(),
                domain.as_ptr(),
                password.as_ptr(),
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
    let job = session
        .job
        .as_ref()
        .ok_or_else(|| "the run has no job object".to_owned())?;
    let process = start_in_job(&info, job)?;
    loop {
        let exited = win32::wait(process.as_handle(), Some(Duration::from_secs(1)))
            .map_err(|err| format!("cannot wait for the command: {err}"))?;
        if exited {
            break;
        }
        if let Err(err) = check_dns_client(session) {
            let _ = job.terminate(1);
            wait_exit(process.as_handle())?;
            return Err(err);
        }
    }
    wait_exit(process.as_handle())
}

pub fn run(config: &Config) -> Result<i32, String> {
    let mut session = Session {
        user: wide(&user_name(&config.name)),
        created: false,
        sid: Vec::new(),
        sid_text: String::new(),
        job: None,
        acl_paths: Vec::new(),
        engine: None,
        dns_port: None,
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
    session.job = Some(
        JobObject::new(Limit::KillOnClose)
            .map_err(|err| format!("cannot create the job object: {err}"))?,
    );
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
    list.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| dx_atomic_fs::write_atomic(list, text.as_bytes()))
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
            let (millis, result) =
                outcome.unwrap_or_else(|| (0, Err(format!("{path} was in no ACL wave"))));
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
        let mut buffer = NetBuffer(null_mut());
        let (mut read, mut total) = (0u32, 0u32);
        // SAFETY: buffer, read, total and resume are valid out-pointers.
        let code = unsafe {
            NetLocalGroupEnum(
                null(),
                0,
                &mut buffer.0,
                MAX_PREFERRED_LENGTH,
                &mut read,
                &mut total,
                &mut resume,
            )
        };
        if !buffer.0.is_null() && (code == 0 || code == ERROR_MORE_DATA) {
            // SAFETY: buffer holds read LOCALGROUP_INFO_0 entries written by NetLocalGroupEnum.
            let entries = unsafe {
                std::slice::from_raw_parts(buffer.0 as *const LOCALGROUP_INFO_0, read as usize)
            };
            for entry in entries {
                // SAFETY: each lgrpi0_name is a NUL-terminated string inside the enum buffer.
                let name = unsafe { wide_text(entry.lgrpi0_name) };
                names.push(name);
            }
        }
        if code != ERROR_MORE_DATA {
            check(code, "listing the local groups")?;
            return Ok(names);
        }
    }
}

/// Whether `pid` may still run. Only a pid Windows does not know counts as gone, so a process
/// we may not open is treated as alive.
fn process_alive(pid: u32) -> bool {
    match win32::open(pid, win32::Access::Wait) {
        Ok(process) => win32::is_running(process.as_handle()).unwrap_or(true),
        Err(err) => err.raw_os_error() != Some(ERROR_INVALID_PARAMETER as i32),
    }
}

/// Reads the DACL of one path. The ACL points into the descriptor.
fn read_acl(path: &str) -> Result<(*mut ACL, LocalMem<c_void>), String> {
    let mut acl: *mut ACL = null_mut();
    let mut descriptor = LocalMem::null();
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
                descriptor.out(),
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
    let (acl, _descriptor) = read_acl(path)?;
    Ok(own_entries(acl, sid.as_ptr() as *const c_void)
        .iter()
        .any(|(_, kind)| *kind == ACCESS_DENIED_ACE_TYPE))
}

/// Deletes every explicit allow and deny entry for `sid` on one path, under the path's lock.
/// `REVOKE_ACCESS` keeps the deny entries, so it cannot undo a `DENY_ACCESS`.
fn remove_access(path: &str, sid: *mut c_void) -> Result<(), String> {
    with_acl_lock(&path_lock_name(path), || remove_access_locked(path, sid))
}

fn remove_access_locked(path: &str, sid: *mut c_void) -> Result<(), String> {
    remove_entries(path, sid, true)
}

/// Deletes the explicit entries for `sid` on `path`, and with `tree` on everything below it.
fn remove_entries(path: &str, sid: *mut c_void, tree: bool) -> Result<(), String> {
    let (acl, descriptor) = read_acl(path)?;
    let mut removed = false;
    for (index, _) in own_entries(acl, sid) {
        // SAFETY: acl is valid and index came from own_entries on it.
        if unsafe { DeleteAce(acl, index) } != 0 {
            removed = true;
        }
    }

    if !removed {
        Ok(())
    } else if tree {
        write_acl(path, descriptor.get(), acl, &|below| {
            remove_entries(below, sid, false)
        })
    } else {
        write_acl_here(path, descriptor.get(), acl)
    }
}
