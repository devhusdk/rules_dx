"""Analysis tests for the Dockerfile strict-tier guard switch in the lint request."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//dockerfile/private/quality:dockerfile.bzl", "real_dockerfile_lint_aspect")

_TIER = str(Label("//config:config_tier"))

_LintActionsInfo = provider(doc = "Actions of the subject under the Dockerfile lint aspect.", fields = {"actions": "list of actions"})

def _subject_impl(ctx):
    return [_LintActionsInfo(actions = ctx.attr.target.actions)]

_subject = rule(
    implementation = _subject_impl,
    doc = "Forwards the actions of target under the Dockerfile lint aspect.",
    attrs = {"target": attr.label(mandatory = True, aspects = [real_dockerfile_lint_aspect])},
)

def _tier_impl(ctx):
    env = analysistest.begin(ctx)
    requests = [
        a.content
        for a in analysistest.target_under_test(env)[_LintActionsInfo].actions
        if a.mnemonic == "FileWrite" and "-real-lint" in a.outputs.to_list()[0].basename and a.outputs.to_list()[0].basename.endswith("-request.json")
    ]
    asserts.equals(env, 1, len(requests), "want one lint request")
    tools = json.decode(requests[0])["tools"] if len(requests) == 1 else {}
    asserts.equals(env, ctx.attr.want, tools.get("hadolint", {}).get("args", []))
    return analysistest.end(env)

_attrs = {"want": attr.string_list(doc = "The hadolint args the lint request must carry.")}

_strict_test = analysistest.make(_tier_impl, attrs = _attrs, config_settings = {_TIER: "strict"})

_standard_test = analysistest.make(_tier_impl, attrs = _attrs, config_settings = {_TIER: "standard"})

def dockerfile_tier_tests(name, target, size):
    """Asserts that the lint request of target turns on the guard on the strict tier only."""
    _subject(name = name + "_subject", target = target, testonly = True)
    _strict_test(name = name + "_strict", size = size, target_under_test = name + "_subject", want = ["strict"])
    _standard_test(name = name + "_standard", size = size, target_under_test = name + "_subject", want = [])
