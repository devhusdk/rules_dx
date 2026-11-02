"""XML source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_source_library")

xml_library = dx_source_library("xml", ["xml", "xsd", "xsl", "xslt", "wsdl"], "XML")
