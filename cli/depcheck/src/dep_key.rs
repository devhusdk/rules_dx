use crate::ecosystem::{normalize_lower, normalize_snake, python};
use dx_ecosystems::Language;

pub(crate) fn normalize_dep_key(eco: Language, raw: &str) -> String {
    match eco {
        Language::Python => python::normalize_py(raw),
        Language::Rust => normalize_lower(raw),
        Language::Go => normalize_lower(raw),
        Language::Java | Language::Kotlin | Language::Scala => normalize_lower(raw),
        Language::Csharp | Language::Fsharp => normalize_lower(raw),
        Language::Cc => normalize_snake(raw),
        Language::Js | Language::Ts => normalize_lower(raw),
        Language::Perl | Language::Zig | Language::Julia | Language::Swift => raw.to_owned(),
        Language::Dart => normalize_lower(raw),
        Language::Php => normalize_lower(raw),
        Language::Ruby | Language::Elixir | Language::Erlang => normalize_lower(raw),
        Language::Fortran | Language::D => normalize_lower(raw),
        Language::Haskell => raw.to_owned(),
        Language::Powershell => normalize_lower(raw),
        Language::Lua => raw.to_owned(),
        Language::Ocaml => raw.to_owned(),
    }
}
