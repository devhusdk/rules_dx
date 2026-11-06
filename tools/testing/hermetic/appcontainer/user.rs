//! The user backend: a new local account per run, its network blocked by WFP filters.
//!
//! Inside an AppContainer the Mount Manager refuses `GetFinalPathNameByHandleW` with a DOS
//! volume name, which Bazel's client needs. A normal user token passes.
use std::ffi::c_void;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::NetworkManagement::NetManagement::{
    LOCALGROUP_MEMBERS_INFO_3, NetLocalGroupAddMembers, NetUserAdd, NetUserDel,
    UF_DONT_EXPIRE_PASSWD, UF_PASSWD_CANT_CHANGE, UF_SCRIPT, USER_INFO_1, USER_PRIV_USER,
};
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FWP_ACTION_BLOCK, FWP_ACTION_PERMIT, FWP_BYTE_BLOB, FWP_CONDITION_FLAG_IS_LOOPBACK,
    FWP_CONDITION_VALUE0, FWP_CONDITION_VALUE0_0, FWP_MATCH_EQUAL, FWP_MATCH_FLAGS_ALL_SET,
    FWP_SECURITY_DESCRIPTOR_TYPE, FWP_UINT8, FWP_UINT32, FWP_VALUE0, FWP_VALUE0_0,
    FWPM_CONDITION_ALE_USER_ID, FWPM_CONDITION_FLAGS, FWPM_FILTER_CONDITION0, FWPM_FILTER0,
    FWPM_LAYER_ALE_AUTH_CONNECT_V4, FWPM_LAYER_ALE_AUTH_CONNECT_V6,
    FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4, FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
    FWPM_SESSION_FLAG_DYNAMIC, FWPM_SESSION0, FwpmEngineClose0, FwpmEngineOpen0, FwpmFilterAdd0,
};
use windows_sys::Win32::Security::Authentication::Identity::RtlGenRandom;
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{GetSecurityDescriptorLength, LookupAccountNameW};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
use windows_sys::Win32::System::Threading::{
    CreateProcessWithLogonW, GetExitCodeProcess, LOGON_WITH_PROFILE, PROCESS_INFORMATION,
    ResumeThread, STARTUPINFOW, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::DeleteProfileW;
use windows_sys::core::GUID;

use crate::args::{Config, command_line, environment, password, user_name};
use crate::sys::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, DENY_ACCESS, GRANT_ACCESS, INFINITE,
    REVOKE_ACCESS, STARTF_USESTDHANDLES, check, kill_on_close_job, set_access, user_defaults, wide,
};

const RPC_C_AUTHN_DEFAULT: u32 = 0xFFFF_FFFF;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_MEMBER_IN_ALIAS: u32 = 1378;
const BLOCK_WEIGHT: u8 = 1;
const PERMIT_WEIGHT: u8 = 10;

/// Everything a run creates. Dropping it undoes whatever exists, in a fixed order.
struct Session {
    user: Vec<u16>,
    created: bool,
    sid: Vec<u8>,
    sid_text: String,
    job: HANDLE,
    acl_paths: Vec<String>,
    engine: HANDLE,
}

impl Session {
    fn sid(&self) -> *mut c_void {
        self.sid.as_ptr() as *mut c_void
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.job.is_null() {
            unsafe { CloseHandle(self.job) };
        }
        for path in &self.acl_paths {
            let _ = set_access(path, self.sid(), REVOKE_ACCESS);
        }
        if self.created {
            if !self.sid_text.is_empty() {
                unsafe { DeleteProfileW(wide(&self.sid_text).as_ptr(), null(), null()) };
            }
            unsafe { NetUserDel(null(), self.user.as_ptr()) };
        }
        if !self.engine.is_null() {
            unsafe { FwpmEngineClose0(self.engine) };
        }
    }
}

fn random_password() -> Result<String, String> {
    let mut bytes = [0u8; 24];
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
    let code = unsafe { NetUserAdd(null(), 1, &info as *const _ as *const u8, &mut bad) };
    if code == ERROR_ACCESS_DENIED {
        return Err(
            "NetUserAdd failed with error 5: the user backend must run elevated (as administrator)"
                .to_owned(),
        );
    }
    check(code, "creating the local account")?;
    session.created = true;

    let mut member = session.user.clone();
    let group = LOCALGROUP_MEMBERS_INFO_3 {
        lgrmi3_domainandname: member.as_mut_ptr(),
    };
    let code = unsafe {
        NetLocalGroupAddMembers(
            null(),
            wide("Users").as_ptr(),
            3,
            &group as *const _ as *const u8,
            1,
        )
    };
    if code != ERROR_MEMBER_IN_ALIAS {
        check(
            code,
            "adding the account to the Users group (it needs the local logon right)",
        )?;
    }
    Ok(())
}

fn look_up_sid(session: &mut Session) -> Result<(), String> {
    let (mut sid_len, mut domain_len, mut kind) = (0u32, 0u32, 0i32);
    unsafe {
        LookupAccountNameW(
            null(),
            session.user.as_ptr(),
            null_mut(),
            &mut sid_len,
            null_mut(),
            &mut domain_len,
            &mut kind,
        )
    };
    let mut sid = vec![0u8; sid_len as usize];
    let mut domain = vec![0u16; domain_len as usize];
    let found = unsafe {
        LookupAccountNameW(
            null(),
            session.user.as_ptr(),
            sid.as_mut_ptr() as *mut c_void,
            &mut sid_len,
            domain.as_mut_ptr(),
            &mut domain_len,
            &mut kind,
        )
    };
    if found == 0 {
        return Err(format!(
            "cannot look up the account SID: {}",
            std::io::Error::last_os_error()
        ));
    }
    session.sid = sid;
    let mut text: *mut u16 = null_mut();
    if unsafe { ConvertSidToStringSidW(session.sid(), &mut text) } == 0 {
        return Err("cannot print the account SID".to_owned());
    }
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
        unsafe { FwpmFilterAdd0(engine, &filter, null_mut(), &mut id) },
        "adding a network filter",
    )
}

/// Blocks every connect and accept of the account except loopback, until the engine closes.
fn block_network(session: &mut Session) -> Result<(), String> {
    let mut wfp: FWPM_SESSION0 = unsafe { std::mem::zeroed() };
    let mut label = wide("dx hermetic");
    wfp.displayData.name = label.as_mut_ptr();
    wfp.flags = FWPM_SESSION_FLAG_DYNAMIC;
    check(
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
        size: unsafe { GetSecurityDescriptorLength(descriptor) },
        data: descriptor as *mut u8,
    };
    let blob_ptr: *mut FWP_BYTE_BLOB = &mut blob;
    let for_user = || {
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
    let mut result = Ok(());
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
    unsafe { LocalFree(descriptor as _) };
    result
}

fn spawn(config: &Config, session: &Session, password: &str) -> Result<i32, String> {
    let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    startup.dwFlags = STARTF_USESTDHANDLES;
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
    let mut line = wide(&command_line(&config.command));
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
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
    if created == 0 {
        return Err(format!(
            "cannot start {} as the run account: {}",
            config.command[0],
            std::io::Error::last_os_error()
        ));
    }
    if unsafe { AssignProcessToJobObject(session.job, info.hProcess) } == 0 {
        unsafe { CloseHandle(info.hThread) };
        unsafe { CloseHandle(info.hProcess) };
        return Err("cannot put the command in the job object".to_owned());
    }
    let mut code = 1u32;
    unsafe {
        ResumeThread(info.hThread);
        CloseHandle(info.hThread);
        WaitForSingleObject(info.hProcess, INFINITE);
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
    }
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
    };
    let password = random_password()?;
    create_user(&mut session, &password)?;
    look_up_sid(&mut session)?;
    if !config.net {
        block_network(&mut session)?;
    }
    for (path, mode) in config
        .grants
        .iter()
        .map(|path| (path, GRANT_ACCESS))
        .chain(config.denies.iter().map(|path| (path, DENY_ACCESS)))
    {
        set_access(path, session.sid(), mode)?;
        session.acl_paths.push(path.clone());
    }
    session.job = kill_on_close_job()?;
    spawn(config, &session, &password)
}
