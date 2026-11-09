"""Owns one OCI image for dx lint."""

load("//core/quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")
load(":image_providers.bzl", "OciImageQualityInfo")

def _oci_image_quality_impl(ctx):
    who = "oci_image_quality " + str(ctx.label) + ": "
    layouts = ctx.attr.image[DefaultInfo].files.to_list()
    if len(layouts) != 1 or not layouts[0].is_directory:
        fail(who + "image must be an OCI layout directory, got " + str(ctx.attr.image.label))
    if len(ctx.files.build_files) == 0:
        fail(who + "build_files must name the BUILD file of the package")
    direct_sources = {"oci_image": depset(ctx.files.build_files)}
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(),
        QualitySourcesInfo(direct_sources = direct_sources),
        OciImageQualityInfo(layout = layouts[0], name = ctx.attr.image.label.name, structure_tests = ctx.files.structure_tests),
    ]

oci_image_quality = rule(
    implementation = _oci_image_quality_impl,
    doc = "Owns the image config of one OCI image for dx lint; findings land on the BUILD file.",
    attrs = {
        "build_files": attr.label_list(allow_files = True, mandatory = True, doc = "The BUILD file of the package."),
        "image": attr.label(mandatory = True, doc = "An OCI image layout directory."),
        "structure_tests": attr.label_list(allow_files = [".yaml", ".yml", ".json"], doc = "container-structure-test configs that dx typecheck runs against the image."),
    },
)
