"""nuget: one rules_dotnet NuGet hub per Paket group of a dx lock."""

load("//core:pins.bzl", "lock_problems")

SOURCE = "https://api.nuget.org/v3/index.json"

MAIN = "main"

def hub_name(hub, group):
    """Returns the repository name of a group: the hub for `main`, `<hub>.<group>` for the others."""
    return hub if group == MAIN else "%s.%s" % (hub, group)

def nuget_packages(group, packages):
    """Returns the rules_dotnet nuget packages of one group of a decoded lock."""
    found = []
    for package in packages:
        sha512 = package.get("extra", {}).get("nuget_sha512")
        if sha512 == None:
            fail("nuget group %s package %s %s has no extra.nuget_sha512" % (group, package["name"], package["version"]))
        found.append({
            "id": package["name"],
            "name": package["name"],
            "sha512": sha512,
            "sources": [SOURCE],
            "version": package["version"],
        })
    return found

def lock_hubs(hub, lock):
    """Returns the rules_dotnet nuget packages of every group of a decoded lock, by repository name."""
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
