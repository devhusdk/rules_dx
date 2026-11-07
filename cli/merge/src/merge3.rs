//! A line-level three-way merge (diff3) of two edits of one ancestor.

use similar::{Algorithm, DiffOp, capture_diff_slices};

/// The names written after the conflict markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Labels<'a> {
    pub ours: &'a str,
    pub base: &'a str,
    pub theirs: &'a str,
}

impl Default for Labels<'_> {
    fn default() -> Self {
        Self {
            ours: "ours",
            base: "base",
            theirs: "theirs",
        }
    }
}

/// The merged text. A conflict is a result, not an error: the text holds the markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merge3 {
    Merged(String),
    Conflicts { text: String, hunks: usize },
}

/// Merges the changes from `ancestor` to `ours` and from `ancestor` to `theirs`, keeping every byte.
pub fn merge3(ancestor: &str, ours: &str, theirs: &str, labels: Labels<'_>) -> Merge3 {
    let base: Vec<&str> = ancestor.split_inclusive('\n').collect();
    let our: Vec<&str> = ours.split_inclusive('\n').collect();
    let their: Vec<&str> = theirs.split_inclusive('\n').collect();
    let to_ours = matches(&base, &our);
    let to_theirs = matches(&base, &their);
    let eol = crate::block::eol(ours);

    let mut out = String::with_capacity(ours.len().max(theirs.len()));
    let mut hunks = 0;
    let (mut i, mut o, mut t) = (0, 0, 0);
    loop {
        let synced = i < base.len() && to_ours[i] == Some(o) && to_theirs[i] == Some(t);
        if synced {
            out.push_str(base[i]);
            (i, o, t) = (i + 1, o + 1, t + 1);
            continue;
        }
        if i >= base.len() && o >= our.len() && t >= their.len() {
            break;
        }
        let next = (i..base.len()).find_map(|k| Some((k, to_ours[k]?, to_theirs[k]?)));
        let (k, oe, te) = next.unwrap_or((base.len(), our.len(), their.len()));
        let b = lines(&base, i, k);
        let x = lines(&our, o, oe);
        let y = lines(&their, t, te);
        if x == b {
            y.iter().for_each(|l| out.push_str(l));
        } else if y == b || x == y {
            x.iter().for_each(|l| out.push_str(l));
        } else {
            hunks += 1;
            marker(&mut out, "<<<<<<<", labels.ours, eol);
            side(&mut out, x, eol);
            marker(&mut out, "|||||||", labels.base, eol);
            side(&mut out, b, eol);
            out.push_str("=======");
            out.push_str(eol);
            side(&mut out, y, eol);
            marker(&mut out, ">>>>>>>", labels.theirs, eol);
        }
        (i, o, t) = (k.max(i), oe.max(o), te.max(t));
    }

    if hunks == 0 {
        Merge3::Merged(out)
    } else {
        Merge3::Conflicts { text: out, hunks }
    }
}

/// For each line of `old`, the index of the equal line in `new` that Myers matched it to.
fn matches(old: &[&str], new: &[&str]) -> Vec<Option<usize>> {
    let mut map = vec![None; old.len()];
    for op in capture_diff_slices(Algorithm::Myers, old, new) {
        if let DiffOp::Equal {
            old_index,
            new_index,
            len,
        } = op
        {
            for n in 0..len {
                if let Some(slot) = map.get_mut(old_index + n) {
                    *slot = Some(new_index + n);
                }
            }
        }
    }
    map
}

fn lines<'s, 'a>(all: &'s [&'a str], from: usize, to: usize) -> &'s [&'a str] {
    all.get(from..to).unwrap_or(&[])
}

fn marker(out: &mut String, sign: &str, label: &str, eol: &str) {
    out.push_str(sign);
    if !label.is_empty() {
        out.push(' ');
        out.push_str(label);
    }
    out.push_str(eol);
}

fn side(out: &mut String, lines: &[&str], eol: &str) {
    lines.iter().for_each(|l| out.push_str(l));
    if lines.last().is_some_and(|l| !l.ends_with('\n')) {
        out.push_str(eol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(a: &str, o: &str, t: &str) -> Merge3 {
        merge3(a, o, t, Labels::default())
    }

    #[test]
    fn edits_that_do_not_overlap_merge() {
        let a = "a\nb\nc\nd\ne\n";
        let o = "A\nb\nc\nd\ne\n";
        let t = "a\nb\nc\nd\nE\n";
        assert_eq!(run(a, o, t), Merge3::Merged("A\nb\nc\nd\nE\n".into()));
    }

    #[test]
    fn insertions_on_both_sides_merge() {
        let a = "a\nb\nc\n";
        let o = "a\nx\nb\nc\n";
        let t = "a\nb\nc\ny\n";
        assert_eq!(run(a, o, t), Merge3::Merged("a\nx\nb\nc\ny\n".into()));
    }

    #[test]
    fn overlapping_edits_conflict() {
        let a = "a\nb\nc\n";
        let o = "a\nB1\nc\n";
        let t = "a\nB2\nc\n";
        let text = "a\n<<<<<<< ours\nB1\n||||||| base\nb\n=======\nB2\n>>>>>>> theirs\nc\n";
        assert_eq!(
            run(a, o, t),
            Merge3::Conflicts {
                text: text.into(),
                hunks: 1
            }
        );
    }

    #[test]
    fn labels_name_the_sides() {
        let labels = Labels {
            ours: "your file",
            base: "old base 1234",
            theirs: "new base",
        };
        let Merge3::Conflicts { text, .. } = merge3("a\n", "b\n", "c\n", labels) else {
            panic!("expected a conflict");
        };
        assert_eq!(
            text,
            "<<<<<<< your file\nb\n||||||| old base 1234\na\n=======\nc\n>>>>>>> new base\n"
        );
    }

    #[test]
    fn crlf_merges_and_stays_crlf() {
        let a = "a\r\nb\r\nc\r\nd\r\n";
        let o = "A\r\nb\r\nc\r\nd\r\n";
        let t = "a\r\nb\r\nc\r\nD\r\n";
        assert_eq!(run(a, o, t), Merge3::Merged("A\r\nb\r\nc\r\nD\r\n".into()));
    }

    #[test]
    fn crlf_conflict_markers_use_crlf() {
        let a = "a\r\nb\r\n";
        let o = "a\r\nx\r\n";
        let t = "a\r\ny\r\n";
        let text =
            "a\r\n<<<<<<< ours\r\nx\r\n||||||| base\r\nb\r\n=======\r\ny\r\n>>>>>>> theirs\r\n";
        assert_eq!(
            run(a, o, t),
            Merge3::Conflicts {
                text: text.into(),
                hunks: 1
            }
        );
    }

    #[test]
    fn ancestor_equal_to_theirs_returns_ours() {
        let a = "\u{feff}key:\tvalue  \nother\n";
        let o = "\u{feff}key:\tchanged  \r\nother\nextra";
        assert_eq!(run(a, o, a), Merge3::Merged(o.into()));
    }

    #[test]
    fn ancestor_equal_to_ours_returns_theirs() {
        let a = "a\nb\n";
        let t = "a\n  b \t\nc";
        assert_eq!(run(a, a, t), Merge3::Merged(t.into()));
    }

    #[test]
    fn same_edit_on_both_sides_is_taken_once() {
        let a = "a\nb\nc\n";
        let o = "a\nB\nc\n";
        assert_eq!(run(a, o, o), Merge3::Merged(o.into()));
    }

    #[test]
    fn last_line_without_eol_in_conflict() {
        let a = "a\nb";
        let o = "a\nx";
        let t = "a\ny";
        let text = "a\n<<<<<<< ours\nx\n||||||| base\nb\n=======\ny\n>>>>>>> theirs\n";
        assert_eq!(
            run(a, o, t),
            Merge3::Conflicts {
                text: text.into(),
                hunks: 1
            }
        );
    }

    #[test]
    fn empty_ancestor() {
        assert_eq!(run("", "a\n", ""), Merge3::Merged("a\n".into()));
        assert_eq!(run("", "a\n", "a\n"), Merge3::Merged("a\n".into()));
        assert_eq!(
            run("", "a\n", "b\n"),
            Merge3::Conflicts {
                text: "<<<<<<< ours\na\n||||||| base\n=======\nb\n>>>>>>> theirs\n".into(),
                hunks: 1
            }
        );
        assert_eq!(run("", "", ""), Merge3::Merged(String::new()));
    }

    #[test]
    fn two_separate_conflicts_count_as_two_hunks() {
        let a = "a\nb\nc\nd\ne\n";
        let o = "A1\nb\nc\nd\nE1\n";
        let t = "A2\nb\nc\nd\nE2\n";
        let Merge3::Conflicts { hunks, .. } = run(a, o, t) else {
            panic!("expected conflicts");
        };
        assert_eq!(hunks, 2);
    }
}
