"""Shared focused environment-plan rule factories."""

load("//env:focused.bzl", "focused_closure_plan", "focused_direct_sources", "focused_js_closure", "focused_js_plan", "focused_npm_store_projection", "focused_python_plan", "focused_python_transitive", "focused_simple_plan", "focused_tsconfig_projection", "focused_typescript_plan", "focused_venv_projection", "focused_write_plan")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label", "starlark_test")

def closure_env_plan_rule(rule_name, info, required, transitive):
    """Builds one closure-family focused environment-plan rule."""

    def _impl(ctx):
        target = ctx.attr.target
        for item in required:
            if item[0] not in target:
                fail(rule_name + ": target has no " + item[1] + ": " + display_label(target.label))
        direct = focused_direct_sources(target)
        sparse = transitive(target)
        parts = focused_closure_plan(direct, sparse, display_label(ctx.attr.target.label))
        plan = parts.plan
        out = focused_write_plan(ctx, plan)
        return [
            DefaultInfo(files = depset([out])),
            info(
                direct_sources = direct,
                has_sources = parts.has_sources,
                has_tests = parts.has_tests,
                source_count = parts.source_count,
                target = plan["target"],
                test_source_count = parts.test_source_count,
                test_sources = parts.test_sources,
                transitive_source_count = parts.transitive_source_count,
                transitive_sources = parts.transitive_sources,
            ),
            DxSubjectInfo(fields = plan),
        ]

    return rule(
        implementation = _impl,
        attrs = {
            "target": attr.label(
                mandatory = True,
            ),
        },
    )

def js_env_plan_rule(rule_name, info, JsInfo, TsConfigInfo = None, with_store = False, with_tsconfig = False):
    """Builds one JS-family focused environment-plan rule."""

    def _impl(ctx):
        target = ctx.attr.target
        if JsInfo not in target:
            fail(rule_name + ": target has no JsInfo: " + display_label(target.label))
        js_info = target[JsInfo]
        closure = focused_js_closure(js_info.transitive_sources.to_list(), js_info.npm_sources.to_list())
        direct = focused_direct_sources(target)
        store = None
        tsconfig = None
        if with_tsconfig:
            plan = focused_typescript_plan(direct, closure, TsConfigInfo in target, display_label(ctx.attr.target.label))
        else:
            plan = focused_js_plan(direct, closure, display_label(ctx.attr.target.label))
        if with_store or with_tsconfig:
            store = focused_npm_store_projection(js_info.npm_package_store_infos.to_list())
        if with_store:
            plan["has_store"] = str(store.has_store)
            plan["store_count"] = str(store.store_count)
        if with_tsconfig:
            if TsConfigInfo in target:
                tsconfig = focused_tsconfig_projection(target[TsConfigInfo].deps.to_list())
            else:
                tsconfig = focused_tsconfig_projection([])
            plan["tsconfig"] = tsconfig.tsconfig
            plan["tsconfig_count"] = str(tsconfig.tsconfig_count)
        out = focused_write_plan(ctx, plan)
        if with_tsconfig:
            extra = info(
                direct_sources = direct,
                has_npm = closure.has_npm,
                has_store = store.has_store,
                has_tsconfig = TsConfigInfo in target,
                npm_source_count = closure.npm_count,
                store_count = store.store_count,
                target = plan["target"],
                transitive_sources = closure.transitive,
                tsconfig = tsconfig.tsconfig,
                tsconfig_count = tsconfig.tsconfig_count,
            )
        elif with_store:
            extra = info(
                direct_sources = direct,
                has_npm = closure.has_npm,
                has_store = store.has_store,
                npm_source_count = closure.npm_count,
                store_count = store.store_count,
                target = plan["target"],
                transitive_sources = closure.transitive,
            )
        else:
            extra = info(
                direct_sources = direct,
                has_npm = closure.has_npm,
                npm_source_count = closure.npm_count,
                target = plan["target"],
                transitive_sources = closure.transitive,
            )
        return [
            DefaultInfo(files = depset([out])),
            extra,
            DxSubjectInfo(fields = plan),
        ]

    return rule(
        implementation = _impl,
        attrs = {
            "target": attr.label(
                mandatory = True,
            ),
        },
    )

def simple_env_plan_rule(rule_name, info, required):
    """Builds one simple-language focused environment-plan rule."""

    def _impl(ctx):
        target = ctx.attr.target
        for item in required:
            if item[0] not in target:
                fail(rule_name + ": target has no " + item[1] + ": " + display_label(target.label))
        direct = focused_direct_sources(target)
        parts = focused_simple_plan(direct, display_label(ctx.attr.target.label))
        plan = parts.plan
        out = focused_write_plan(ctx, plan)
        return [
            DefaultInfo(files = depset([out])),
            info(
                direct_sources = direct,
                has_sources = parts.has_sources,
                source_count = parts.source_count,
                target = plan["target"],
            ),
            DxSubjectInfo(fields = plan),
        ]

    return rule(
        implementation = _impl,
        attrs = {
            "target": attr.label(
                mandatory = True,
            ),
        },
    )

def python_env_plan_rule(rule_name, info, PyInfo, WheelsInfo, wheels_aspect):
    """Builds the focused Python environment-plan rule."""

    def _impl(ctx):
        target = ctx.attr.target
        if PyInfo not in target:
            fail(rule_name + ": target has no PyInfo: " + display_label(target.label))
        py_info = target[PyInfo]
        imports = sorted(py_info.imports.to_list())
        sparse = focused_python_transitive(py_info.transitive_sources.to_list())
        direct = focused_direct_sources(target)
        if WheelsInfo not in target:
            fail(rule_name + ": wheels aspect missing on target: " + display_label(target.label))
        wheel_count = len(target[WheelsInfo].wheels.to_list())
        parts = focused_python_plan(direct, sparse, imports, wheel_count, display_label(ctx.attr.target.label))
        plan = parts.plan
        venv = focused_venv_projection(target)
        plan["has_venv"] = str(venv.has_venv)
        plan["venv"] = venv.venv
        out = focused_write_plan(ctx, plan)
        return [
            DefaultInfo(files = depset([out])),
            info(
                direct_sources = direct,
                has_venv = venv.has_venv,
                has_wheels = parts.has_wheels,
                imports = imports,
                target = plan["target"],
                transitive_sources = sparse,
                venv = venv.venv,
                wheel_count = wheel_count,
            ),
            DxSubjectInfo(fields = plan),
        ]

    return rule(
        implementation = _impl,
        attrs = {
            "target": attr.label(
                aspects = [wheels_aspect],
                mandatory = True,
            ),
        },
    )

def env_plan_tests(name, subjects, expected, **kwargs):
    """Declares one pinned-observation focused environment-plan test."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = subjects,
        expected_observations = expected,
        **kwargs
    )
