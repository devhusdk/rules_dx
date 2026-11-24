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

const _: () = assert!(size_of::<usize>() == 8, "dx supports only 64-bit Windows");

const OBJECT_NAME_INFORMATION: OBJECT_INFORMATION_CLASS = 1;
const SYSTEM_EXTENDED_HANDLE_INFORMATION: SYSTEM_INFORMATION_CLASS = 64;
const HANDLE_ENTRY_WORDS: usize = 5;
const HANDLE_HEADER_WORDS: usize = 2;
const QUERY_ATTEMPTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemHandle {
    pub pid: usize,
    pub handle: usize,
}

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

pub(crate) fn unicode_string(buffer: &[u64]) -> io::Result<String> {
    let outside = || io::Error::other("the UNICODE_STRING points outside its buffer");
    let header = buffer.first().copied().ok_or_else(outside)?;
    let length = usize::try_from(header & 0xffff).map_err(|error| {
        io::Error::other(format!(
            "the UNICODE_STRING length does not fit usize: {error}"
        ))
    })?;
    let pointer = buffer.get(1).copied().ok_or_else(outside)?;
    let pointer = usize::try_from(pointer).map_err(|error| {
        io::Error::other(format!(
            "the UNICODE_STRING pointer does not fit usize: {error}"
        ))
    })?;
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

pub(crate) fn parse_handles(words: &[u64]) -> io::Result<Vec<SystemHandle>> {
    let count = usize::try_from(words.first().copied().unwrap_or(0)).map_err(io::Error::other)?;
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
    entries
        .iter()
        .take(count)
        .map(|&[_, pid, handle, _, _]| {
            Ok(SystemHandle {
                pid: usize::try_from(pid).map_err(io::Error::other)?,
                handle: usize::try_from(handle).map_err(io::Error::other)?,
            })
        })
        .collect()
}

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

pub fn object_type_name(handle: BorrowedHandle<'_>) -> io::Result<String> {
    object_text(handle, ObjectTypeInformation)
}

pub fn object_name(handle: BorrowedHandle<'_>) -> io::Result<String> {
    object_text(handle, OBJECT_NAME_INFORMATION)
}
