use super::*;
use prost_types::source_code_info::Location;
use prost_types::{
    EnumValueDescriptorProto, FieldDescriptorProto, MessageOptions, MethodDescriptorProto,
    SourceCodeInfo,
};

const P: &str = "protobuf:shop:";

fn location(path: &[i32], line: i32, comment: &str) -> Location {
    Location {
        path: path.to_vec(),
        span: vec![line, 0, 1],
        leading_comments: if comment.is_empty() {
            None
        } else {
            Some(comment.to_owned())
        },
        ..Location::default()
    }
}

fn field(name: &str, number: i32, ty: Type, type_name: &str) -> FieldDescriptorProto {
    FieldDescriptorProto {
        name: Some(name.to_owned()),
        number: Some(number),
        label: Some(Label::Optional as i32),
        r#type: Some(ty as i32),
        type_name: (!type_name.is_empty()).then(|| type_name.to_owned()),
        ..FieldDescriptorProto::default()
    }
}

fn file() -> FileDescriptorProto {
    let mut tags = field("tags", 2, Type::String, "");
    tags.label = Some(Label::Repeated as i32);
    let attrs = FieldDescriptorProto {
        label: Some(Label::Repeated as i32),
        ..field("attrs", 3, Type::Message, ".shop.v1.Order.AttrsEntry")
    };
    let entry = DescriptorProto {
        name: Some("AttrsEntry".to_owned()),
        field: vec![
            field("key", 1, Type::String, ""),
            field("value", 2, Type::Int32, ""),
        ],
        options: Some(MessageOptions {
            map_entry: Some(true),
            ..MessageOptions::default()
        }),
        ..DescriptorProto::default()
    };
    let order = DescriptorProto {
        name: Some("Order".to_owned()),
        field: vec![
            field("id", 1, Type::String, ""),
            tags,
            attrs,
            field("status", 4, Type::Enum, ".shop.v1.Status"),
        ],
        nested_type: vec![
            entry,
            DescriptorProto {
                name: Some("Line".to_owned()),
                ..DescriptorProto::default()
            },
        ],
        enum_type: vec![EnumDescriptorProto {
            name: Some("Kind".to_owned()),
            value: vec![EnumValueDescriptorProto {
                name: Some("KIND_A".to_owned()),
                number: Some(0),
                ..EnumValueDescriptorProto::default()
            }],
            ..EnumDescriptorProto::default()
        }],
        ..DescriptorProto::default()
    };
    let status = EnumDescriptorProto {
        name: Some("Status".to_owned()),
        value: vec![
            EnumValueDescriptorProto {
                name: Some("STATUS_UNKNOWN".to_owned()),
                number: Some(0),
                ..EnumValueDescriptorProto::default()
            },
            EnumValueDescriptorProto {
                name: Some("STATUS_OPEN".to_owned()),
                number: Some(1),
                ..EnumValueDescriptorProto::default()
            },
        ],
        ..EnumDescriptorProto::default()
    };
    let service = ServiceDescriptorProto {
        name: Some("Shop".to_owned()),
        method: vec![
            MethodDescriptorProto {
                name: Some("Get".to_owned()),
                input_type: Some(".shop.v1.Order".to_owned()),
                output_type: Some(".shop.v1.Order".to_owned()),
                ..MethodDescriptorProto::default()
            },
            MethodDescriptorProto {
                name: Some("Watch".to_owned()),
                input_type: Some(".shop.v1.Order".to_owned()),
                output_type: Some(".shop.v1.Order".to_owned()),
                client_streaming: Some(true),
                server_streaming: Some(true),
                ..MethodDescriptorProto::default()
            },
        ],
        ..ServiceDescriptorProto::default()
    };
    FileDescriptorProto {
        name: Some("shop/v1/shop.proto".to_owned()),
        package: Some("shop.v1".to_owned()),
        message_type: vec![order],
        enum_type: vec![status],
        service: vec![service],
        source_code_info: Some(SourceCodeInfo {
            location: vec![
                location(&[4, 0], 5, " An order.\n\n Second paragraph.\n"),
                location(&[4, 0, 2, 0], 6, " The order id.\n"),
                location(&[4, 0, 2, 2], 8, " Free-form attributes.\n"),
                location(&[4, 0, 3, 1], 12, " One line of an order.\n"),
                location(&[4, 0, 4, 0], 14, " Order kinds.\n"),
                location(&[4, 0, 4, 0, 2, 0], 15, " First kind.\n"),
                location(&[5, 0], 20, " Order status.\n"),
                location(&[5, 0, 2, 1], 22, " The order is open.\n"),
                location(&[6, 0], 26, " The shop API.\n"),
                location(&[6, 0, 2, 0], 27, " Reads one order.\n"),
                location(&[6, 0, 2, 1], 29, " Streams orders.\n"),
            ],
        }),
        ..FileDescriptorProto::default()
    }
}

fn options() -> Options {
    let mut options = Options {
        package: "shop".to_owned(),
        out: "out".to_owned(),
        descriptor: "in".to_owned(),
        ..Options::default()
    };
    options.srcs.insert(
        "shop/v1/shop.proto".to_owned(),
        "api/shop/v1/shop.proto".to_owned(),
    );
    options
}

fn encode(files: Vec<FileDescriptorProto>) -> Vec<u8> {
    FileDescriptorSet { file: files }.encode_to_vec()
}

fn convert_one() -> DocIr {
    convert(&encode(vec![file()]), &options()).unwrap()
}

fn symbol<'a>(shard: &'a DocIr, qualified: &str) -> &'a Symbol {
    let id = format!("{P}{qualified}");
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} is missing: {:?}", ids(shard)))
}

fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

fn members(symbol: &Symbol) -> Vec<&str> {
    symbol
        .relations
        .as_ref()
        .map(|r| r.member_of.iter().map(String::as_str).collect())
        .unwrap_or_default()
}

#[test]
fn every_kind_maps_with_its_comment_and_line() {
    let shard = convert_one();
    let order = symbol(&shard, "shop.v1.Order");
    assert_eq!(order.kind, SymbolKind::Class as i32);
    assert_eq!(order.doc_markdown, "An order.\n\nSecond paragraph.");
    assert_eq!(order.signature_text, "message Order");
    let source = order.source.as_ref().unwrap();
    assert_eq!(source.file, "api/shop/v1/shop.proto");
    assert_eq!(source.line, 6);
    assert!(!source.generated);
    assert!(members(order).is_empty());

    let id = symbol(&shard, "shop.v1.Order.id");
    assert_eq!(id.kind, SymbolKind::Field as i32);
    assert_eq!(id.signature_text, "string id = 1;");
    assert_eq!(id.doc_markdown, "The order id.");
    assert_eq!(members(id), vec![format!("{P}shop.v1.Order")]);

    let tags = symbol(&shard, "shop.v1.Order.tags");
    assert_eq!(tags.signature_text, "repeated string tags = 2;");

    let status = symbol(&shard, "shop.v1.Status");
    assert_eq!(status.kind, SymbolKind::Enum as i32);
    let open = symbol(&shard, "shop.v1.Status.STATUS_OPEN");
    assert_eq!(open.kind, SymbolKind::EnumVariant as i32);
    assert_eq!(open.signature_text, "STATUS_OPEN = 1;");
    assert_eq!(open.doc_markdown, "The order is open.");
    assert_eq!(members(open), vec![format!("{P}shop.v1.Status")]);

    let service = symbol(&shard, "shop.v1.Shop");
    assert_eq!(service.kind, SymbolKind::Interface as i32);
    let get = symbol(&shard, "shop.v1.Shop.Get");
    assert_eq!(get.kind, SymbolKind::Method as i32);
    assert_eq!(
        get.signature_text,
        "rpc Get(shop.v1.Order) returns (shop.v1.Order);"
    );
    assert_eq!(get.doc_markdown, "Reads one order.");
    assert_eq!(get.params[0].r#type, "shop.v1.Order");
}

#[test]
fn nested_types_and_enums_are_members_of_their_message() {
    let shard = convert_one();
    let line = symbol(&shard, "shop.v1.Order.Line");
    assert_eq!(line.kind, SymbolKind::Class as i32);
    assert_eq!(line.doc_markdown, "One line of an order.");
    assert_eq!(members(line), vec![format!("{P}shop.v1.Order")]);
    let kind = symbol(&shard, "shop.v1.Order.Kind");
    assert_eq!(kind.kind, SymbolKind::Enum as i32);
    assert_eq!(members(kind), vec![format!("{P}shop.v1.Order")]);
    let value = symbol(&shard, "shop.v1.Order.Kind.KIND_A");
    assert_eq!(value.doc_markdown, "First kind.");
    assert_eq!(members(value), vec![format!("{P}shop.v1.Order.Kind")]);
}

#[test]
fn map_fields_show_as_maps_and_entry_messages_are_hidden() {
    let shard = convert_one();
    let attrs = symbol(&shard, "shop.v1.Order.attrs");
    assert_eq!(attrs.signature_text, "map<string, int32> attrs = 3;");
    assert_eq!(attrs.doc_markdown, "Free-form attributes.");
    assert!(!ids(&shard).iter().any(|id| id.contains("AttrsEntry")));
}

#[test]
fn streaming_rpcs_are_marked() {
    let shard = convert_one();
    let watch = symbol(&shard, "shop.v1.Shop.Watch");
    assert_eq!(
        watch.signature_text,
        "rpc Watch(stream shop.v1.Order) returns (stream shop.v1.Order);"
    );
    assert_eq!(
        watch.returns.as_ref().unwrap().r#type,
        "stream shop.v1.Order"
    );
}

#[test]
fn missing_comments_are_flagged_incomplete_not_invented() {
    let shard = convert_one();
    let status_field = symbol(&shard, "shop.v1.Order.status");
    assert!(status_field.doc_markdown.is_empty());
    assert_eq!(status_field.extensions[0].key, KEY_INCOMPLETE);
    assert!(
        !symbol(&shard, "shop.v1.Order")
            .extensions
            .iter()
            .any(|e| e.key == KEY_INCOMPLETE)
    );
}

#[test]
fn a_descriptor_without_source_info_is_rejected() {
    let mut bare = file();
    bare.source_code_info = None;
    let error = convert(&encode(vec![bare]), &options()).unwrap_err();
    assert!(error.contains("--include_source_info"), "{error}");
}

#[test]
fn a_file_outside_the_sources_is_rejected() {
    let mut other = options();
    other.srcs.clear();
    other
        .srcs
        .insert("x.proto".to_owned(), "x.proto".to_owned());
    let error = convert(&encode(vec![file()]), &other).unwrap_err();
    assert!(error.contains("shop/v1/shop.proto"), "{error}");
}

#[test]
fn trailing_comments_document_an_item_without_a_leading_one() {
    let mut trailing = file();
    let info = trailing.source_code_info.as_mut().unwrap();
    info.location.push(Location {
        path: vec![4, 0, 2, 3],
        span: vec![9, 0, 1],
        trailing_comments: Some(" The status.\n".to_owned()),
        ..Location::default()
    });
    let shard = convert(&encode(vec![trailing]), &options()).unwrap();
    assert_eq!(
        symbol(&shard, "shop.v1.Order.status").doc_markdown,
        "The status."
    );
}

#[test]
fn symbols_are_sorted_and_the_shard_validates() {
    let shard = convert_one();
    let mut sorted = ids(&shard);
    sorted.sort();
    assert_eq!(ids(&shard), sorted);
    documentation_ir::validate_shard(&shard).unwrap();
}

#[test]
fn generated_sources_are_marked() {
    let mut generated = options();
    generated.generated.insert("shop/v1/shop.proto".to_owned());
    let shard = convert(&encode(vec![file()]), &generated).unwrap();
    assert!(
        symbol(&shard, "shop.v1.Order")
            .source
            .as_ref()
            .unwrap()
            .generated
    );
}

#[test]
fn parse_collects_sources_and_rejects_bad_flags() {
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let parsed = parse(&args(&[
        "--descriptor",
        "d",
        "--package",
        "p",
        "--out",
        "o",
        "--src",
        "a.proto=x/a.proto",
        "--generated",
        "a.proto",
    ]))
    .unwrap();
    assert_eq!(parsed.srcs["a.proto"], "x/a.proto");
    assert!(parsed.generated.contains("a.proto"));
    assert!(parse(&args(&["--bogus", "1"])).is_err());
    assert!(parse(&args(&["--src", "nopath"])).is_err());
    assert!(
        parse(&args(&["--package", "p"]))
            .unwrap_err()
            .contains("--descriptor")
    );
}
