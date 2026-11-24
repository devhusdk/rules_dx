use std::fmt;
use std::io::{self, Write};

pub fn write_line(out: &mut dyn Write, args: fmt::Arguments<'_>) -> io::Result<()> {
    match out.write_fmt(args).and_then(|()| out.write_all(b"\n")) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}

pub fn eprint_line(args: fmt::Arguments<'_>) {
    match write_line(&mut io::stderr().lock(), args) {
        Ok(()) | Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Failing(io::ErrorKind);

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(self.0.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn write_line_appends_a_newline() {
        let mut out = Vec::new();
        write_line(&mut out, format_args!("dx {}", 1)).expect("write");
        assert_eq!(out, b"dx 1\n");
    }

    #[test]
    fn write_line_treats_a_closed_pipe_as_success() {
        write_line(&mut Failing(io::ErrorKind::BrokenPipe), format_args!("dx"))
            .expect("broken pipe is success");
    }

    #[test]
    fn write_line_reports_other_errors() {
        let error = write_line(
            &mut Failing(io::ErrorKind::PermissionDenied),
            format_args!("dx"),
        )
        .expect_err("permission denied");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
