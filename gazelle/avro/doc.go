// Package avro generates one avro_library per directory that holds schema
// files. Dependencies come from the import idl and import schema statements of .avdl files
// and resolve to the library owning the file, next to the including file or
// from the repository root.
//
// When MODULE.bazel depends on rules_cc, rules_java or rules_rust, it also
// generates the matching <library>_cc, _java and _rust code generation rules.
package avro
