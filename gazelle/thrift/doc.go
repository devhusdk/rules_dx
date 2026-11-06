// Package thrift generates one thrift_library per directory that holds schema
// files. Dependencies come from the include "<path>" statements
// and resolve to the library owning the file, next to the including file or
// from the repository root.
package thrift
