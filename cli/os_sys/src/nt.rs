//! ntdll queries: process command lines and the system handle table.

#![expect(
    unsafe_code,
    reason = "these ntdll queries have no Win32 or std equivalent"
)]

use std::io;
use std::os::windows::io::{AsRawHandle, BorrowedHandle};
use windows_sys::Wdk::Foundation::{
    NtQueryObject, OBJECT_INFORMATION_CLASS, ObjectTypeInformation,
};
use windows_sys::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS,
};
use windows_sys::Wdk::System::Threading::{
    NtQueryInformationProcess, ProcessCommandLineInformation,
};
use windows_sys::Win32::Foundation::{
    NTSTATUS, STATUS_BUFFER_OVERFLOW, STATUS_BUFFER_TOO_SMALL, STATUS_INFO_LENGTH_MISMATCH,
};

// The handle table and UNICODE_STRING are read as 64-bit words.
const _: () = assert!(size_of::<usize>() == 8, "dx supports only 64-bit Windows");

const OBJECT_NAME_INFORMATION: OBJECT_INFORMATION_CLASS = 1;
const SYSTEM_EXTENDED_HANDLE_INFORMATION: SYSTEM_INFORMATION_CLASS = 64;
/// Words in one SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX: object, pid, handle, then 16 bytes of flags.
const HANDLE_ENTRY_WORDS: usize = 5;
/// Words before the first handle entry: NumberOfHandles and a reserved word.
const HANDLE_HEADER_WORDS: usize = 2;
const QUERY_ATTEMPTS: usize = 8;

/// One open handle in some process, from the system handle table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemHandle {
    pub pid: usize,
    /// The handle's value inside process `pid`.
    pub handle: usize,
}

/// Runs an ntdll query that fills a caller buffer, growing the buffer while the call asks for more.
/// `call` gets the buffer, its byte length and an out-parameter for the length needed.
pub(crate) fn query(
    what: &str,
    initial_bytes: u32,
    mut call: impl FnMut(&mut [u64], u32, &mut u32) -> NTSTATUS,
) -> io::Result<Vec<u64>> {
    let mut size = initial_bytes;
    for _ in 0..QUERY_ATTEMPTS {
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let mut needed = 0u32;
        let status = call(&mut buffer, size, &mut needed);
        if matches!(
            status,
            STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_OVERFLOW | STATUS_BUFFER_TOO_SMALL
        ) {
            size = needed.max(size.saturating_mul(2));
            continue;
        }
        if status < 0 {
            return Err(io::Error::other(format!(
                "{what} failed with NTSTATUS {status:#x}"
            )));
        }
        return Ok(buffer);
    }
    Err(io::Error::other(format!(
        "{what} kept outgrowing its buffer"
    )))
}

/// Decodes the UNICODE_STRING at the start of `buffer`, whose text must lie inside `buffer`.
pub(crate) fn unicode_string(buffer: &[u64]) -> io::Result<String> {
    let outside = || io::Error::other("the UNICODE_STRING points outside its buffer");
    let header = buffer.first().copied().ok_or_else(outside)?;
    let length = (header & 0xffff) as usize;
    let pointer = buffer.get(1).copied().ok_or_else(outside)? as usize;
    if length == 0 {
        return Ok(String::new());
    }
    let start = pointer
        .checked_sub(buffer.as_ptr() as usize)
        .ok_or_else(outside)?;
    let end = start.checked_add(length).ok_or_else(outside)?;
    let bytes: Vec<u8> = buffer.iter().flat_map(|word| word.to_le_bytes()).collect();
    let text = bytes.get(start..end).ok_or_else(outside)?;
    let (pairs, _) = text.as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    Ok(String::from_utf16_lossy(&units))
}

/// The command line of `process`, which needs query access.
pub fn command_line(process: BorrowedHandle<'_>) -> io::Result<String> {
    let buffer = query("NtQueryInformationProcess", 512, |buffer, len, needed| {
        // SAFETY: query passes the buffer's byte length; the process handle is live.
        unsafe {
            NtQueryInformationProcess(
                process.as_raw_handle(),
                ProcessCommandLineInformation,
                buffer.as_mut_ptr().cast(),
                len,
                needed,
            )
        }
    })?;
    unicode_string(&buffer)
}

/// Splits a SYSTEM_HANDLE_INFORMATION_EX buffer into its entries.
pub(crate) fn parse_handles(words: &[u64]) -> io::Result<Vec<SystemHandle>> {
    let count = words.first().copied().unwrap_or(0) as usize;
    let (entries, _) = words
        .get(HANDLE_HEADER_WORDS..)
        .unwrap_or_default()
        .as_chunks::<HANDLE_ENTRY_WORDS>();
    if count > entries.len() {
        return Err(io::Error::other(format!(
            "the handle table lists {count} handles but holds {}",
            entries.len()
        )));
    }
    Ok(entries
        .iter()
        .take(count)
        .map(|&[_, pid, handle, _, _]| SystemHandle {
            pid: pid as usize,
            handle: handle as usize,
        })
        .collect())
}

/// Every open handle in every process.
pub fn system_handles() -> io::Result<Vec<SystemHandle>> {
    let words = query(
        "NtQuerySystemInformation",
        1 << 20,
        |buffer, len, needed| {
            // SAFETY: query passes the buffer's byte length.
            unsafe {
                NtQuerySystemInformation(
                    SYSTEM_EXTENDED_HANDLE_INFORMATION,
                    buffer.as_mut_ptr().cast(),
                    len,
                    needed,
                )
            }
        },
    )?;
    parse_handles(&words)
}

fn object_text(handle: BorrowedHandle<'_>, class: OBJECT_INFORMATION_CLASS) -> io::Result<String> {
    let buffer = query("NtQueryObject", 4096, |buffer, len, needed| {
        // SAFETY: query passes the buffer's byte length; the handle is live.
        unsafe {
            NtQueryObject(
                handle.as_raw_handle(),
                class,
                buffer.as_mut_ptr().cast(),
                len,
                needed,
            )
        }
    })?;
    unicode_string(&buffer)
}

/// The kernel object type of `handle`, such as `File` or `ALPC Port`.
pub fn object_type_name(handle: BorrowedHandle<'_>) -> io::Result<String> {
    object_text(handle, ObjectTypeInformation)
}

/// The kernel object name of `handle`. Some pipes never answer this query; check the type first.
pub fn object_name(handle: BorrowedHandle<'_>) -> io::Result<String> {
    object_text(handle, OBJECT_NAME_INFORMATION)
}
