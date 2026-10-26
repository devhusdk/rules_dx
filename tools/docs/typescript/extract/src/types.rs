//! Renders TypeDoc type and signature JSON as TypeScript source text.

use serde_json::Value;

const FLAG_OPTIONAL: &str = "isOptional";
const FLAG_REST: &str = "isRest";
const FLAG_READONLY: &str = "isReadonly";
const FLAG_STATIC: &str = "isStatic";
const FLAG_ABSTRACT: &str = "isAbstract";

/// Returns the string field `key` of `node`, or "".
pub fn text<'a>(node: &'a Value, key: &str) -> &'a str {
    node.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Returns the array field `key` of `node`, or an empty slice.
pub fn list<'a>(node: &'a Value, key: &str) -> &'a [Value] {
    node.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Reports whether `node.flags[flag]` is true.
pub fn flag(node: &Value, flag: &str) -> bool {
    node.get("flags")
        .and_then(|flags| flags.get(flag))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Renders one TypeDoc type object.
pub fn render_type(node: &Value) -> Result<String, String> {
    let kind = text(node, "type");
    match kind {
        "intrinsic" | "unknown" => Ok(text(node, "name").to_owned()),
        "literal" => literal(node),
        "reference" => reference(node),
        "array" => Ok(format!(
            "{}[]",
            wrapped(field(node, "elementType")?, Slot::Postfix)?
        )),
        "union" => joined(list(node, "types"), " | ", Slot::Union),
        "intersection" => joined(list(node, "types"), " & ", Slot::Intersection),
        "tuple" => {
            let items = list(node, "elements")
                .iter()
                .map(render_type)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", items.join(", ")))
        }
        "namedTupleMember" => {
            let optional = if node.get("isOptional").and_then(Value::as_bool) == Some(true) {
                "?"
            } else {
                ""
            };
            Ok(format!(
                "{}{optional}: {}",
                text(node, "name"),
                render_type(field(node, "element")?)?
            ))
        }
        "optional" => Ok(format!(
            "{}?",
            wrapped(field(node, "elementType")?, Slot::Postfix)?
        )),
        "rest" => Ok(format!(
            "...{}",
            wrapped(field(node, "elementType")?, Slot::Postfix)?
        )),
        "typeOperator" => Ok(format!(
            "{} {}",
            text(node, "operator"),
            wrapped(field(node, "target")?, Slot::Postfix)?
        )),
        "indexedAccess" => Ok(format!(
            "{}[{}]",
            wrapped(field(node, "objectType")?, Slot::Postfix)?,
            render_type(field(node, "indexType")?)?
        )),
        "conditional" => Ok(format!(
            "{} extends {} ? {} : {}",
            wrapped(field(node, "checkType")?, Slot::Postfix)?,
            wrapped(field(node, "extendsType")?, Slot::Postfix)?,
            render_type(field(node, "trueType")?)?,
            render_type(field(node, "falseType")?)?
        )),
        "query" => Ok(format!(
            "typeof {}",
            render_type(field(node, "queryType")?)?
        )),
        "predicate" => predicate(node),
        "reflection" => reflection(field(node, "declaration")?),
        "mapped" => mapped(node),
        "templateLiteral" => template_literal(node),
        "inferred" => {
            let constraint = match node.get("constraint") {
                Some(constraint) => format!(" extends {}", render_type(constraint)?),
                None => String::new(),
            };
            Ok(format!("infer {}{constraint}", text(node, "name")))
        }
        other => Err(format!("unsupported TypeDoc type kind '{other}'")),
    }
}

fn field<'a>(node: &'a Value, key: &str) -> Result<&'a Value, String> {
    node.get(key)
        .ok_or_else(|| format!("TypeDoc type '{}' has no '{key}'", text(node, "type")))
}

#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Postfix,
    Union,
    Intersection,
}

fn wrapped(node: &Value, slot: Slot) -> Result<String, String> {
    let rendered = render_type(node)?;
    let kind = text(node, "type");
    let function = kind == "reflection"
        && node
            .get("declaration")
            .map(|declaration| !list(declaration, "signatures").is_empty())
            .unwrap_or(false);
    let needs = match slot {
        Slot::Postfix => {
            matches!(
                kind,
                "union"
                    | "intersection"
                    | "conditional"
                    | "typeOperator"
                    | "query"
                    | "inferred"
                    | "optional"
                    | "rest"
            ) || function
        }
        Slot::Union => matches!(kind, "conditional") || function,
        Slot::Intersection => matches!(kind, "union" | "conditional") || function,
    };
    if needs {
        Ok(format!("({rendered})"))
    } else {
        Ok(rendered)
    }
}

fn joined(items: &[Value], separator: &str, slot: Slot) -> Result<String, String> {
    let parts = items
        .iter()
        .map(|item| wrapped(item, slot))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(parts.join(separator))
}

fn literal(node: &Value) -> Result<String, String> {
    match node.get("value") {
        Some(Value::String(value)) => Ok(Value::String(value.clone()).to_string()),
        Some(Value::Number(value)) => Ok(value.to_string()),
        Some(Value::Bool(value)) => Ok(value.to_string()),
        Some(Value::Null) | None => Ok("null".to_owned()),
        Some(object @ Value::Object(_)) => {
            let sign = if object.get("negative").and_then(Value::as_bool) == Some(true) {
                "-"
            } else {
                ""
            };
            Ok(format!("{sign}{}n", text(object, "value")))
        }
        Some(Value::Array(_)) => Err("TypeDoc literal type holds an array".to_owned()),
    }
}

fn reference(node: &Value) -> Result<String, String> {
    let name = {
        let qualified = text(node, "qualifiedName");
        let shown = text(node, "name");
        if qualified.is_empty() || node.get("package").is_none() {
            shown
        } else {
            qualified
        }
    };
    let arguments = list(node, "typeArguments");
    if arguments.is_empty() {
        return Ok(name.to_owned());
    }
    let rendered = arguments
        .iter()
        .map(render_type)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{name}<{}>", rendered.join(", ")))
}

fn predicate(node: &Value) -> Result<String, String> {
    let asserts = if node.get("asserts").and_then(Value::as_bool) == Some(true) {
        "asserts "
    } else {
        ""
    };
    match node.get("targetType") {
        Some(target) => Ok(format!(
            "{asserts}{} is {}",
            text(node, "name"),
            render_type(target)?
        )),
        None => Ok(format!("{asserts}{}", text(node, "name"))),
    }
}

fn mapped(node: &Value) -> Result<String, String> {
    let readonly = match text(node, "readonlyModifier") {
        "+" => "readonly ",
        "-" => "-readonly ",
        _ => "",
    };
    let optional = match text(node, "optionalModifier") {
        "+" => "?",
        "-" => "-?",
        _ => "",
    };
    let rename = match node.get("nameType") {
        Some(name) => format!(" as {}", render_type(name)?),
        None => String::new(),
    };
    Ok(format!(
        "{{ {readonly}[{} in {}{rename}]{optional}: {} }}",
        text(node, "parameter"),
        render_type(field(node, "parameterType")?)?,
        render_type(field(node, "templateType")?)?
    ))
}

fn template_literal(node: &Value) -> Result<String, String> {
    let mut out = String::from("`");
    out.push_str(text(node, "head"));
    for pair in list(node, "tail") {
        let items = pair
            .as_array()
            .ok_or_else(|| "TypeDoc template literal tail is not a pair".to_owned())?;
        let (Some(part), Some(tail)) = (items.first(), items.get(1)) else {
            return Err("TypeDoc template literal tail is not a pair".to_owned());
        };
        out.push_str(&format!(
            "${{{}}}{}",
            render_type(part)?,
            tail.as_str().unwrap_or("")
        ));
    }
    out.push('`');
    Ok(out)
}

fn reflection(declaration: &Value) -> Result<String, String> {
    let signatures = list(declaration, "signatures");
    if let Some(signature) = signatures.first() {
        if signatures.len() == 1 {
            return function_type(signature);
        }
        let parts = signatures
            .iter()
            .map(|signature| Ok(format!("{{ {} }}", call_signature(signature)?)))
            .collect::<Result<Vec<_>, String>>()?;
        return Ok(parts.join(" & "));
    }
    object_literal(declaration)
}

/// Renders the members of a declaration as an object literal type.
pub fn object_literal(declaration: &Value) -> Result<String, String> {
    let mut members = Vec::new();
    for child in list(declaration, "children") {
        members.push(member(child)?);
    }
    for index in list(declaration, "indexSignatures") {
        let parameter = list(index, "parameters")
            .first()
            .ok_or_else(|| "TypeDoc index signature has no key".to_owned())?;
        members.push(format!(
            "[{}: {}]: {}",
            text(parameter, "name"),
            render_type(field(parameter, "type")?)?,
            render_type(field(index, "type")?)?
        ));
    }
    if members.is_empty() {
        return Ok("{}".to_owned());
    }
    Ok(format!("{{ {} }}", members.join("; ")))
}

fn member(child: &Value) -> Result<String, String> {
    let signatures = list(child, "signatures");
    if !signatures.is_empty() {
        let parts = signatures
            .iter()
            .map(|signature| {
                Ok(format!(
                    "{}{}",
                    member_prefix(child),
                    call_signature(signature)?
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        return Ok(parts.join("; "));
    }
    let optional = if flag(child, FLAG_OPTIONAL) { "?" } else { "" };
    Ok(format!(
        "{}{}{optional}: {}",
        member_prefix(child),
        text(child, "name"),
        render_type(field(child, "type")?)?
    ))
}

fn member_prefix(child: &Value) -> &'static str {
    if flag(child, FLAG_READONLY) {
        "readonly "
    } else {
        ""
    }
}

/// Renders the type parameter list of a declaration or signature.
pub fn type_parameters(node: &Value) -> Result<String, String> {
    let items = list(node, "typeParameters");
    if items.is_empty() {
        return Ok(String::new());
    }
    let mut parts = Vec::new();
    for item in items {
        let mut part = text(item, "name").to_owned();
        if let Some(constraint) = item.get("type") {
            part.push_str(&format!(" extends {}", render_type(constraint)?));
        }
        if let Some(default) = item.get("default") {
            part.push_str(&format!(" = {}", render_type(default)?));
        }
        parts.push(part);
    }
    Ok(format!("<{}>", parts.join(", ")))
}

/// Renders the parameter list of one signature, with parentheses.
pub fn parameters(signature: &Value) -> Result<String, String> {
    let mut parts = Vec::new();
    for parameter in list(signature, "parameters") {
        let name = text(parameter, "name");
        let rendered = render_type(field(parameter, "type")?)?;
        let rest = if flag(parameter, FLAG_REST) {
            "..."
        } else {
            ""
        };
        let defaulted = parameter.get("defaultValue").is_some();
        let optional = if flag(parameter, FLAG_OPTIONAL) && !defaulted {
            "?"
        } else {
            ""
        };
        let default = match parameter.get("defaultValue").and_then(Value::as_str) {
            Some(value) => format!(" = {value}"),
            None => String::new(),
        };
        parts.push(format!("{rest}{name}{optional}: {rendered}{default}"));
    }
    Ok(format!("({})", parts.join(", ")))
}

/// Renders `name<T>(a: A): R` for one signature.
pub fn call_signature(signature: &Value) -> Result<String, String> {
    let generics = type_parameters(signature)?;
    let params = parameters(signature)?;
    let returns = match signature.get("type") {
        Some(ty) => format!(": {}", render_type(ty)?),
        None => String::new(),
    };
    Ok(format!(
        "{}{generics}{params}{returns}",
        text(signature, "name")
    ))
}

fn function_type(signature: &Value) -> Result<String, String> {
    let generics = type_parameters(signature)?;
    let params = parameters(signature)?;
    let returns = match signature.get("type") {
        Some(ty) => render_type(ty)?,
        None => "void".to_owned(),
    };
    Ok(format!("{generics}{params} => {returns}"))
}

/// Renders the modifier words that precede a class member name.
pub fn modifiers(node: &Value) -> String {
    let mut out = String::new();
    if flag(node, FLAG_STATIC) {
        out.push_str("static ");
    }
    if flag(node, FLAG_ABSTRACT) {
        out.push_str("abstract ");
    }
    if flag(node, FLAG_READONLY) {
        out.push_str("readonly ");
    }
    out
}

/// Renders a comment summary or block content as Markdown.
pub fn comment_text(parts: &[Value]) -> String {
    let mut out = String::new();
    for part in parts {
        let body = text(part, "text");
        if text(part, "kind") == "inline-tag" {
            out.push('`');
            out.push_str(body);
            out.push('`');
        } else {
            out.push_str(body);
        }
    }
    out.trim().to_owned()
}
