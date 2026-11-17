use super::{Ignores, comment_text, nearby_reason, take_word};
use crate::LcovError;

pub fn find_ignores(path: &str, source: &str) -> Result<Ignores, LcovError> {
    let lines: Vec<&str> = source.lines().collect();
    let mut ignores = Ignores::default();
    let mut open: Option<(usize, u32, String)> = None;
    for ((index, line), number) in lines.iter().enumerate().zip(1_u32..) {
        let lineno = index + 1;
        let comment = comment_text(path, line);
        for (pos, marker) in comment.match_indices("LCOV_EXCL") {
            let Some(rest) = comment.get(pos + marker.len()..) else {
                continue;
            };
            if take_word(rest, "_LINE") {
                let reason = nearby_reason(path, "LINE directive", lineno, &lines)?;
                ignores.singles.insert(number, reason);
            } else if take_word(rest, "_START") {
                let reason = nearby_reason(path, "START directive", lineno, &lines)?;
                if open.is_some() {
                    return Err(LcovError::NestedStart {
                        path: path.to_string(),
                        lineno,
                    });
                }
                open = Some((lineno, number, reason));
            } else if take_word(rest, "_STOP") {
                nearby_reason(path, "STOP directive", lineno, &lines)?;
                match open.take() {
                    Some((_, start, start_reason)) => {
                        ignores.ranges.push((start, number, start_reason));
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
    if let Some((start, _, _)) = open {
        return Err(LcovError::UnclosedStart {
            path: path.to_string(),
            start,
        });
    }
    Ok(ignores)
}
