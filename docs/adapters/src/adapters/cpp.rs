use documentation_ir::proto::DocIr;
use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;

use crate::common::{make_symbol, shard, SymbolSpec};
use crate::types::AdapterError;
use crate::CPP_DOXYGEN_PIN;

const ROOT: &str = "doxygen";

#[derive(Default)]
struct Member {
    kind: String,
    name: String,
    brief: String,
    file: String,
    line: u64,
}

impl Member {
    fn new(kind: String) -> Self {
        Member {
            kind,
            line: 1,
            ..Member::default()
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Brief,
}

pub fn normalize_cpp(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let mut symbols = Vec::new();
    for member in members(input)? {
        if member.name.is_empty() {
            continue;
        }
        let signature = format!("{} {}", member.kind, member.name);
        symbols.push(make_symbol(SymbolSpec {
            language: "cpp",
            package,
            qualified: &member.name,
            kind_name: &member.kind,
            signature,
            doc: member.brief,
            file: member.file,
            line: member.line,
        })?);
    }
    shard("cpp", package, symbols)
}

fn attribute(
    element: &BytesStart<'_>,
    name: &str,
    decoder: Decoder,
) -> Result<Option<String>, AdapterError> {
    for candidate in element.attributes() {
        let candidate = candidate
            .map_err(|err| AdapterError::InvalidJson(format!("unreadable {name}: {err}")))?;
        if candidate.key.as_ref() == name.as_bytes() {
            return candidate
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                .map(|value| Some(value.into_owned()))
                .map_err(|err| AdapterError::InvalidJson(format!("unreadable {name}: {err}")));
        }
    }
    Ok(None)
}

fn named_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "apos" => Some('\''),
        "gt" => Some('>'),
        "lt" => Some('<'),
        "quot" => Some('"'),
        _ => None,
    }
}

fn resolve(reference: &BytesRef<'_>) -> String {
    if let Some(ch) = reference.resolve_char_ref().unwrap_or(None) {
        return ch.to_string();
    }
    let name = String::from_utf8_lossy(reference.clone().into_inner().as_ref()).into_owned();
    named_entity(&name).map_or_else(|| format!("&{name};"), |ch| ch.to_string())
}

fn squash(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_paragraph(paragraphs: &mut Vec<String>, buffer: &mut String) {
    let text = squash(buffer);
    buffer.clear();
    if !text.is_empty() {
        paragraphs.push(text);
    }
}

fn members(input: &str) -> Result<Vec<Member>, AdapterError> {
    let mut reader = Reader::from_str(input);
    let decoder = reader.decoder();
    let mut members: Vec<Member> = Vec::new();
    let mut open: Option<(usize, Member)> = None;
    let mut field: Option<(Field, usize)> = None;
    let mut buffer = String::new();
    let mut paragraphs: Vec<String> = Vec::new();
    let mut depth = 0usize;
    loop {
        let (element, empty) = match reader.read_event() {
            Ok(Event::Start(element)) => (element, false),
            Ok(Event::Empty(element)) => (element, true),
            Ok(Event::End(element)) => {
                let level = depth.saturating_sub(1);
                if open.as_ref().is_some_and(|(base, _)| level == *base) {
                    if let Some((_, member)) = open.take() {
                        members.push(member);
                    }
                    field = None;
                } else if let Some((which, base)) = field {
                    if level == base {
                        close_field(
                            which,
                            open.as_mut().map(|(_, member)| member),
                            &mut buffer,
                            &mut paragraphs,
                        );
                        field = None;
                    } else if level == base + 1
                        && which == Field::Brief
                        && element.name().as_ref() == b"para"
                    {
                        push_paragraph(&mut paragraphs, &mut buffer);
                    }
                }
                buffer.clear();
                depth = level;
                continue;
            }
            Ok(Event::Text(text)) => {
                if field.is_some() {
                    let decoded = text
                        .xml_content(XmlVersion::Implicit1_0)
                        .map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
                    buffer.push_str(&decoded);
                }
                continue;
            }
            Ok(Event::GeneralRef(value)) => {
                if field.is_some() {
                    buffer.push_str(&resolve(&value));
                }
                continue;
            }
            Ok(Event::Eof) => break,
            Ok(_) => continue,
            Err(err) => return Err(AdapterError::InvalidJson(err.to_string())),
        };
        let name = element.name();
        if depth == 0 {
            if name.as_ref() != ROOT.as_bytes() {
                return Err(AdapterError::InvalidJson(format!("missing {ROOT} root")));
            }
            let found =
                attribute(&element, "version", decoder)?.unwrap_or_else(|| "unpinned".to_owned());
            if found != CPP_DOXYGEN_PIN {
                return Err(AdapterError::VersionMismatch {
                    expected: CPP_DOXYGEN_PIN.to_owned(),
                    found,
                });
            }
        } else if name.as_ref() == b"memberdef" {
            if open.is_some() {
                return Err(AdapterError::InvalidJson(
                    "memberdef inside memberdef".to_owned(),
                ));
            }
            let kind =
                attribute(&element, "kind", decoder)?.unwrap_or_else(|| "function".to_owned());
            let member = Member::new(kind);
            if empty {
                members.push(member);
            } else {
                open = Some((depth, member));
            }
        } else if let Some((base, member)) = open.as_mut() {
            if *base + 1 != depth {
                depth += usize::from(!empty);
                continue;
            }
            match name.as_ref() {
                b"location" if member.file.is_empty() => {
                    member.file = attribute(&element, "file", decoder)?.unwrap_or_default();
                    member.line = attribute(&element, "line", decoder)?
                        .and_then(|line| line.parse().ok())
                        .unwrap_or(1);
                }
                b"name" if !empty => field = Some((Field::Name, depth)),
                b"briefdescription" if !empty => field = Some((Field::Brief, depth)),
                b"para" if !empty && field.is_some_and(|(which, _)| which == Field::Brief) => {
                    push_paragraph(&mut paragraphs, &mut buffer);
                }
                _ => {}
            }
        }
        depth += usize::from(!empty);
    }
    Ok(members)
}

fn close_field(
    which: Field,
    member: Option<&mut Member>,
    buffer: &mut String,
    paragraphs: &mut Vec<String>,
) {
    if let Some(member) = member {
        match which {
            Field::Name => member.name = squash(buffer),
            Field::Brief => {
                push_paragraph(paragraphs, buffer);
                member.brief = paragraphs.join("\n\n");
            }
        }
    }
    paragraphs.clear();
}
