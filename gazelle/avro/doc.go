// Package avro generates one avro_library per directory that holds schema
// files. Dependencies come from the import idl and import schema statements of .avdl files
// and resolve to the library owning the file, next to the including file or
// from the repository root.
package avro
