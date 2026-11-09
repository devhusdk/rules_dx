// Package ownership generates the source ownership rules that dx quality needs
// for files recognised by name: dockerfile_library, cmake_library,
// bicep_library and asciidoc_library. Each rule owns the matching files of one
// directory and has no dependencies.
package ownership
