// Package ownership generates the source ownership rules that dx quality needs
// for files recognised by name: dockerfile_library, cmake_library,
// bicep_library, asciidoc_library, make_library, terraform_library and
// helm_library, and for YAML files recognised by their first 4 KiB:
// ansible_library for playbooks, cloudformation_library for templates,
// kubernetes_library for manifests and api_spec_library for OpenAPI and
// AsyncAPI documents. Each rule owns the matching files of one directory and has no dependencies. It also generates an oci_image_quality
// for each oci_image rule in a BUILD file, named after the image with a
// _quality suffix.
package ownership
