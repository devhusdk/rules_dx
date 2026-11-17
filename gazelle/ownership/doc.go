// Package ownership generates the source ownership rules that dx quality needs
// for files recognised by name: dockerfile_library, cmake_library,
// bicep_library, asciidoc_library, make_library, terraform_library and
// helm_library, fortran_library for Fortran files without a program unit in
// their first 4 KiB when every use statement among them names an intrinsic
// module, and for YAML files recognised by their first 4 KiB:
// ansible_library for playbooks, cloudformation_library for templates,
// kubernetes_library for manifests and api_spec_library for OpenAPI and
// AsyncAPI documents. Each rule owns the matching files of one directory and has no dependencies. It also generates an oci_image_quality
// for each oci_image rule in a BUILD file, named after the image with a
// _quality suffix, and an appimage_package named <name>_appimage for each
// <name>.desktop file with a <name>.png or <name>.svg icon and a notice file
// beside it, packaging the one *_binary rule of the directory, and an
// msix_package named <binary>_msix for a Package.appxmanifest with an identity
// name, publisher, version and PNG logo path and a notice file beside it, packaging
// the same one *_binary rule.
package ownership
