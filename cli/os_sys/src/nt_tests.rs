#![cfg(windows)]

use crate::nt::{
    SystemHandle, command_line, object_name, object_type_name, parse_handles, query,
    system_handles, unicode_string,
};
use crate::win32::{Access, open};
use std::os::windows::io::{AsHandle, AsRawHandle};
use windows_sys::Win32::Foundation::STATUS_INFO_LENGTH_MISMATCH;

#[test]
fn this_process_command_line_names_its_executable() {
    let me = open(std::process::id(), Access::Query).expect("open self");
    let line = command_line(me.as_handle()).expect("command line");
    let exe = std::env::current_exe().expect("exe");
    let stem = exe
        .file_stem()
        .expect("stem")
        .to_string_lossy()
        .into_owned();
    assert!(line.contains(&stem), "{line}");
}

#[test]
fn a_file_handle_has_no_command_line() {
    let file = std::fs::File::open(std::env::current_exe().expect("exe")).expect("open");
    assert!(command_line(file.as_handle()).is_err());
}

#[test]
fn the_handle_table_holds_an_open_file_of_this_process() {
    let file = std::fs::File::open(std::env::current_exe().expect("exe")).expect("open");
    let own = SystemHandle {
        pid: std::process::id() as usize,
        handle: file.as_raw_handle() as usize,
    };
    assert!(system_handles().expect("handles").contains(&own));
}

#[test]
fn a_file_has_the_file_type_and_a_device_name() {
    let file = std::fs::File::open(std::env::current_exe().expect("exe")).expect("open");
    assert_eq!(object_type_name(file.as_handle()).expect("type"), "File");
    assert!(
        object_name(file.as_handle())
            .expect("name")
            .starts_with("\\Device\\")
    );
}

#[test]
fn a_unicode_string_is_read_from_inside_its_buffer() {
    let mut buffer = [4, 0, u64::from_le_bytes([b'd', 0, b'x', 0, 0, 0, 0, 0])];
    let text = buffer.as_ptr() as u64 + 16;
    let [_, pointer, _] = &mut buffer;
    *pointer = text;
    assert_eq!(unicode_string(&buffer).expect("text"), "dx");
}

#[test]
fn an_empty_unicode_string_is_empty() {
    assert_eq!(unicode_string(&[0, 0]).expect("text"), "");
}

#[test]
fn a_unicode_string_pointing_outside_its_buffer_is_an_error() {
    assert!(unicode_string(&[]).is_err());
    assert!(unicode_string(&[4]).is_err());
    assert!(unicode_string(&[4, 8]).is_err());
    let mut buffer = [4u64, 0];
    let past_end = buffer.as_ptr() as u64 + 16;
    let [_, pointer] = &mut buffer;
    *pointer = past_end;
    assert!(unicode_string(&buffer).is_err());
    let [_, pointer] = &mut buffer;
    *pointer = u64::MAX;
    assert!(unicode_string(&buffer).is_err());
}

#[test]
fn a_handle_table_with_more_entries_than_it_holds_is_an_error() {
    assert!(parse_handles(&[1, 0]).is_err());
    assert_eq!(parse_handles(&[]).expect("empty"), Vec::new());
    assert_eq!(
        parse_handles(&[1, 0, 9, 4, 8, 0, 0]).expect("one"),
        vec![SystemHandle { pid: 4, handle: 8 }]
    );
}

#[test]
fn a_query_grows_its_buffer_to_the_length_asked_for() {
    let mut sizes = Vec::new();
    let buffer = query("test", 8, |buffer, len, needed| {
        sizes.push((buffer.len(), len));
        *needed = 40;
        if len < 40 {
            STATUS_INFO_LENGTH_MISMATCH
        } else {
            0
        }
    })
    .expect("query");
    assert_eq!(buffer.len(), 5);
    assert_eq!(sizes, vec![(1, 8), (5, 40)]);
}

#[test]
fn a_query_that_never_fits_is_an_error() {
    let err = query("test", 8, |_, _, _| STATUS_INFO_LENGTH_MISMATCH).expect_err("never fits");
    assert!(err.to_string().contains("kept outgrowing"));
}
