"""nuget: one rules_dotnet NuGet hub per Paket group of a dx lock."""

load("//core:pins.bzl", "lock_problems")

SOURCE = "https://api.nuget.org/v3/index.json"

MAIN = "main"

def hub_name(hub, group):
    """Returns the repository name of a group: the hub for `main`, `<hub>.<group>` for the others."""
    return hub if group == MAIN else "%s.%s" % (hub, group)

def nuget_packages(group, packages):
    """Returns the rules_dotnet nuget_repo packages of one group of a decoded lock."""
    found = []
    for package in packages:
        extra = package.get("extra", {})
        for key in ("nuget_frameworks", "nuget_sha512"):
            if key not in extra:
                fail("nuget group %s package %s %s has no extra.%s" % (group, package["name"], package["version"], key))
        found.append({
            "dependencies": {tfm: package["deps"] for tfm in extra["nuget_frameworks"]},
            "framework_list": [],
            "id": package["name"],
            "name": package["name"],
            "sha512": extra["nuget_sha512"],
            "sources": [SOURCE],
            "targeting_pack_overrides": [],
            "tools": {tfm: tools for tfm, tools in extra.get("nuget_tools", {}).items() if tfm in extra["nuget_frameworks"]},
            "version": package["version"],
        })
    return found

def lock_hubs(hub, lock):
    """Returns the nuget_repo packages of every group of a decoded lock, by repository name."""
    problems = lock_problems(lock)
    if problems:
        fail("nuget hub %r: %s" % (hub, "; ".join(problems)))
    if lock.get("packages"):
        fail("nuget hub %r: packages belong under groups" % hub)
    return {
        hub_name(hub, group): nuget_packages(group, packages)
        for group, packages in sorted(lock.get("groups", {}).items())
        if packages
    }
