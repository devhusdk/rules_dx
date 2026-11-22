use crate::ecosystem::{normalize_lower, normalize_snake, python};
use dx_ecosystems::Language;

pub(crate) fn normalize_dep_key(eco: Language, raw: &str) -> String {
    match eco {
        Language::Python => python::normalize_py(raw),
        Language::Rust
        | Language::Go
        | Language::Java
        | Language::Kotlin
        | Language::Scala
        | Language::Csharp
        | Language::Fsharp
        | Language::Js
        | Language::Ts
        | Language::Dart
        | Language::Php
        | Language::Ruby
        | Language::Elixir
        | Language::Erlang
        | Language::Fortran
        | Language::D
        | Language::Powershell => normalize_lower(raw),
        Language::Cc => normalize_snake(raw),
        Language::Perl
        | Language::Zig
        | Language::Julia
        | Language::Swift
        | Language::Haskell
        | Language::Lua
        | Language::Ocaml
        | Language::Ansible => raw.to_owned(),
    }
}
