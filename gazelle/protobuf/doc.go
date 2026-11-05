// Package protobuf generates one proto_library per directory that holds
// .proto files. The target is named <directory>_proto, or <repo>_proto at
// the repo root. Imports resolve against the repo-relative path of each
// source, google/protobuf/*.proto resolves to @protobuf//:*_proto, and any
// other import fails generation.
package protobuf
