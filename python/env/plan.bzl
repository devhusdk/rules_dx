"""Focused Python environment plan."""

load("@aspect_rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("//env:plan_factory.bzl", "python_env_plan_rule")
load("//python/env:aspect.bzl", "PythonEnvWheelsInfo", "dx_python_env_wheels_aspect")

PythonEnvPlanInfo = provider(
    doc = "Provider-derived focused Python target environment plan.",
    fields = {
        "direct_sources": "Sorted basenames of direct Python sources.",
        "has_venv": "Whether RunEnvironmentInfo projects a materialized .venv.",
        "has_wheels": "Whether the transitive wheel closure is non-empty.",
        "imports": "Sorted configured import roots from authoritative PyInfo.",
        "target": "Display label of the planned wrapper target.",
        "transitive_sources": "Sorted basenames of transitive first-party sources.",
        "venv": "Workspace-relative VIRTUAL_ENV path, else empty.",
        "wheel_count": "Number of wheels in the transitive PyWheelsInfo closure.",
    },
)

python_env_plan = python_env_plan_rule(
    rule_name = "python_env_plan",
    info = PythonEnvPlanInfo,
    PyInfo = _PyInfo,
    WheelsInfo = PythonEnvWheelsInfo,
    wheels_aspect = dx_python_env_wheels_aspect,
)
