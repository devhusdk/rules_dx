"""The jar tools a consumer downloads from @dx_prebuilt, with the source package and deploy jar of each."""

load("//cli/prebuilt:tools.bzl", "JAR_TOOLS", "TOOLS")

visibility("private")

def _jar(tool):
    source = Label(TOOLS[tool].source)
    return struct(
        tool = tool,
        package = source.package,
        deploy_jar = "//{}:{}_deploy.jar".format(source.package, source.name),
    )

PREBUILT_JARS = [_jar(tool) for tool in JAR_TOOLS]
