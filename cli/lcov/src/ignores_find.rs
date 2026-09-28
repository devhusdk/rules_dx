use super::{comment_text, nearby_reason, take_word, Ignores};
use crate::LcovError;

pub fn find_ignores(path: &str, source: &str) -> Result<Ignores, LcovError> {
    let lines: Vec<&str> = source.lines().collect();
    let mut ignores = Ignores::default();
    let mut open: Option<(usize, String)> = None;
    for (index, line) in lines.iter().enumerate() {
        let lineno = index + 1;
        let comment = comment_text(path, line);
        for (pos, _) in comment.match_indices("LCOV_EXCL") {
            let rest = &comment[pos + "LCOV_EXCL".len()..];
            if take_word(rest, "_LINE") {
                let reason = nearby_reason(path, "LINE directive", lineno, &lines)?;
                ignores.singles.insert(lineno as u32, reason);
            } else if take_word(rest, "_START") {
                let reason = nearby_reason(path, "START directive", lineno, &lines)?;
                if open.is_some() {
                    return Err(LcovError::NestedStart {
                        path: path.to_string(),
                        lineno,
                    });
                }
                open = Some((lineno, reason));
            } else if take_word(rest, "_STOP") {
                let reason = nearby_reason(path, "STOP directive", lineno, &lines)?;
                match open.take() {
                    Some((start, start_reason)) => {
                        ignores
                            .ranges
                            .push((start as u32, lineno as u32, start_reason));
                        let _ = reason;
                    }
                    None => {
                        return Err(LcovError::StopWithoutStart {
                            path: path.to_string(),
                            lineno,
                        });
                    }
                }
            } else {
                return Err(LcovError::UnrecognizedDirective {
                    path: path.to_string(),
                    lineno,
                });
            }
        }
    }
    if let Some((start, _)) = open {
        return Err(LcovError::UnclosedStart {
            path: path.to_string(),
            start,
        });
    }
    Ok(ignores)
}
