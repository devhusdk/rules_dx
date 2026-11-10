"""Source ownership rules for dx quality."""

load("//core:wrapper.bzl", "dx_source_library")

cue_library = dx_source_library("cue", ["cue"], "CUE")
gherkin_library = dx_source_library("gherkin", ["feature"], "Gherkin")
html_template_library = dx_source_library("html_template", ["html", "htm", "jinja", "jinja2", "j2", "njk", "hbs", "gohtml", "tmpl"], "HTML template")
jsonnet_library = dx_source_library("jsonnet", ["jsonnet", "libsonnet"], "Jsonnet")
jupyter_library = dx_source_library("jupyter", ["ipynb"], "Jupyter notebook")
latex_library = dx_source_library("latex", ["tex", "sty", "cls"], "LaTeX")
nix_library = dx_source_library("nix", ["nix"], "Nix")
pkl_library = dx_source_library("pkl", ["pkl"], "Pkl")
typst_library = dx_source_library("typst", ["typ"], "Typst")
xml_library = dx_source_library("xml", ["xml", "xsd", "xsl", "xslt", "wsdl"], "XML")
yaml_library = dx_source_library("yaml", ["yaml", "yml"], "YAML")
