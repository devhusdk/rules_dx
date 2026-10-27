use crate::vuln::VersionScope;
use std::cmp::Ordering;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Segment {
    Num(u128),
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GemVersion {
    segments: Vec<Segment>,
}

pub fn split_gem_lock_version(text: &str) -> (&str, Option<&str>) {
    match text.split_once('-') {
        Some((version, platform)) if !platform.is_empty() => (version, Some(platform)),
        _ => (text, None),
    }
}

fn parse_gem_version(text: &str) -> Option<GemVersion> {
    let text = text.trim();
    if text.is_empty() || text.len() > 256 {
        return None;
    }
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (text, None),
    };
    let mut pieces = core.split('.');
    let first = pieces.next()?;
    if first.is_empty() || !first.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut raw: Vec<&str> = vec![first];
    for piece in pieces {
        if piece.is_empty() || !piece.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return None;
        }
        raw.push(piece);
    }
    let mut pre_pieces: Vec<&str> = Vec::new();
    if let Some(pre) = pre {
        for piece in pre.split('.') {
            if piece.is_empty()
                || !piece
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return None;
            }
            pre_pieces.push(piece);
        }
    }
    let mut segments = Vec::new();
    for piece in &raw {
        push_segments(piece, &mut segments)?;
    }
    if pre.is_some() {
        segments.push(Segment::Text("pre".to_owned()));
        for piece in &pre_pieces {
            push_segments(piece, &mut segments)?;
        }
    }
    Some(GemVersion { segments })
}

fn push_segments(piece: &str, out: &mut Vec<Segment>) -> Option<()> {
    let bytes = piece.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        let digit = bytes[start].is_ascii_digit();
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() == digit {
            end += 1;
        }
        let run = &piece[start..end];
        if digit {
            out.push(Segment::Num(run.parse().ok()?));
        } else if run.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            out.push(Segment::Text(run.to_owned()));
        }
        start = end;
    }
    Some(())
}

impl GemVersion {
    fn canonical(&self) -> Vec<Segment> {
        let split = self
            .segments
            .iter()
            .position(|segment| matches!(segment, Segment::Text(_)))
            .unwrap_or(self.segments.len());
        let mut head = self.segments[..split].to_vec();
        let mut tail = self.segments[split..].to_vec();
        for part in [&mut head, &mut tail] {
            while matches!(part.last(), Some(Segment::Num(0))) {
                part.pop();
            }
        }
        head.extend(tail);
        head
    }

    fn compare(&self, other: &GemVersion) -> Ordering {
        let left = self.canonical();
        let right = other.canonical();
        let zero = Segment::Num(0);
        for index in 0..left.len().max(right.len()) {
            let l = left.get(index).unwrap_or(&zero);
            let r = right.get(index).unwrap_or(&zero);
            let order = match (l, r) {
                (Segment::Num(a), Segment::Num(b)) => a.cmp(b),
                (Segment::Text(a), Segment::Text(b)) => a.cmp(b),
                (Segment::Text(_), Segment::Num(_)) => Ordering::Less,
                (Segment::Num(_), Segment::Text(_)) => Ordering::Greater,
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }

    fn bump(&self) -> Option<GemVersion> {
        let mut segments = self.segments.clone();
        while matches!(segments.last(), Some(Segment::Text(_))) {
            segments.pop();
        }
        if segments.len() > 1 {
            segments.pop();
        }
        match segments.last_mut()? {
            Segment::Num(value) => *value = value.checked_add(1)?,
            Segment::Text(_) => return None,
        }
        Some(GemVersion { segments })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Op {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    Pessimistic,
}

fn parse_requirement(scope: &str) -> Option<Vec<(Op, GemVersion)>> {
    if scope.len() > 4096 {
        return None;
    }
    let mut out = Vec::new();
    for part in scope.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        let (op, rest) = if let Some(rest) = part.strip_prefix(">=") {
            (Op::Ge, rest)
        } else if let Some(rest) = part.strip_prefix("<=") {
            (Op::Le, rest)
        } else if let Some(rest) = part.strip_prefix("!=") {
            (Op::Ne, rest)
        } else if let Some(rest) = part.strip_prefix("~>") {
            (Op::Pessimistic, rest)
        } else if let Some(rest) = part.strip_prefix('>') {
            (Op::Gt, rest)
        } else if let Some(rest) = part.strip_prefix('<') {
            (Op::Lt, rest)
        } else if let Some(rest) = part.strip_prefix('=') {
            (Op::Eq, rest)
        } else {
            (Op::Eq, part)
        };
        out.push((op, parse_gem_version(rest)?));
    }
    Some(out)
}

fn satisfies(op: Op, bound: &GemVersion, version: &GemVersion) -> Option<bool> {
    let order = version.compare(bound);
    Some(match op {
        Op::Eq => order == Ordering::Equal,
        Op::Ne => order != Ordering::Equal,
        Op::Gt => order == Ordering::Greater,
        Op::Lt => order == Ordering::Less,
        Op::Ge => order != Ordering::Less,
        Op::Le => order != Ordering::Greater,
        Op::Pessimistic => {
            order != Ordering::Less && version.compare(&bound.bump()?) == Ordering::Less
        }
    })
}

pub fn ruby_assess(scope: &str, version: &str, platform: Option<&str>) -> VersionScope {
    let scope = scope.trim();
    let version = version.trim();
    if scope.is_empty() || version.is_empty() {
        return VersionScope::Unaffected;
    }
    let display = match platform {
        Some(platform) => format!("{version}-{platform}"),
        None => version.to_owned(),
    };
    if scope == version || scope == display {
        return VersionScope::Affected;
    }
    if scope == "*" {
        return VersionScope::Affected;
    }
    let Some(requirement) = parse_requirement(scope) else {
        return VersionScope::Incomplete;
    };
    let Some(locked) = parse_gem_version(version) else {
        return VersionScope::Incomplete;
    };
    for (op, bound) in &requirement {
        match satisfies(*op, bound, &locked) {
            Some(true) => {}
            Some(false) => return VersionScope::Unaffected,
            None => return VersionScope::Incomplete,
        }
    }
    VersionScope::Affected
}

pub fn ruby_in_scope(scope: &str, version: &str) -> bool {
    let (bare, platform) = split_gem_lock_version(version.trim());
    ruby_assess(scope, bare, platform) == VersionScope::Affected
}
