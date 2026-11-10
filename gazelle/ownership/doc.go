// Package ownership generates the source ownership rules that dx quality needs
// for files recognised by name: dockerfile_library, cmake_library,
// bicep_library and asciidoc_library. Each rule owns the matching files of one
// directory and has no dependencies. It also generates an oci_image_quality
// for each oci_image rule in a BUILD file, named after the image with a
// _quality suffix.
package ownership
