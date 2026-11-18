"""The composer rules of a dx lock, on top of the shared `lock_problems`."""

def composer_problems(lock):
    """Returns why a decoded dx lock that `lock_problems` accepts is not a composer lock: one message per problem, empty when it is."""
    problems = []
    if lock.get("ecosystem") != "composer":
        problems.append("has ecosystem %r, not composer" % lock.get("ecosystem"))
    for package in lock["packages"]:
        extra = package.get("extra", {})
        if type(extra) != "dict" or type(extra.get("composer_manifest")) != "dict":
            problems.append("package %s has no composer_manifest object" % package["name"])
    return problems
