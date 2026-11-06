use crate::ecosystem::{normalize_lower, normalize_snake, python};
use crate::Ecosystem;

pub(crate) fn normalize_dep_key(eco: Ecosystem, raw: &str) -> String {
    match eco {
        Ecosystem::Python => python::normalize_py(raw),
        Ecosystem::Rust => normalize_lower(raw),
        Ecosystem::Go => normalize_lower(raw),
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => normalize_lower(raw),
        Ecosystem::Csharp | Ecosystem::Fsharp => normalize_lower(raw),
        Ecosystem::Cc => normalize_snake(raw),
        Ecosystem::Js | Ecosystem::Ts => normalize_lower(raw),
        Ecosystem::Perl | Ecosystem::Zig | Ecosystem::Julia | Ecosystem::Swift => raw.to_owned(),
        Ecosystem::Dart => normalize_lower(raw),
        Ecosystem::Php => normalize_lower(raw),
        Ecosystem::Ruby | Ecosystem::Elixir | Ecosystem::Erlang => normalize_lower(raw),
        Ecosystem::Fortran | Ecosystem::D => normalize_lower(raw),
        Ecosystem::Haskell => raw.to_owned(),
    }
}
