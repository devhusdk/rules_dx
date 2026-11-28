"""Unit tests for the OCI image context hooks."""

load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")
load("//core/quality:families/oci_image.bzl", "HOOKS")
load("//deploy/private:image_providers.bzl", "OciImageQualityInfo")

visibility("private")

_STAGES = [{"tool": "container_structure_test"}, {"tool": "trivy_image_config"}]

_LAYOUT = struct(path = "bazel-out/bin/app/image")

_TEST = struct(path = "app/structure.yaml")

def _image(structure_tests):
    return {OciImageQualityInfo: OciImageQualityInfo(layout = _LAYOUT, name = "image", structure_tests = structure_tests)}

def _structure(target):
    return HOOKS["container_structure_test"](target, None, _STAGES, "typecheck", "what", None)

def oci_image_hooks_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("an image without structure tests drops the stage", _structure(_image([])).resolved, [{"tool": "trivy_image_config"}]),
            expect_equal("a target without an image drops the stage", _structure({}).resolved, [{"tool": "trivy_image_config"}]),
            expect_equal("structure tests keep the stage", _structure(_image([_TEST])).resolved, None),
            expect_equal("the layout comes before the configs", _structure(_image([_TEST])).options, {"args": [_LAYOUT.path, _TEST.path]}),
        ],
    )
