// Package capnproto generates one capnp_library per directory that holds schema
// files. Dependencies come from the import "<path>" expressions; a leading slash names a path from the repository root and /capnp/ files are bundled
// and resolve to the library owning the file, next to the including file or
// from the repository root.
package capnproto
