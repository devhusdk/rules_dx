"""Anonymous ghcr.io pull tokens for repository rules."""

visibility(["//core/...", "//tools/testing/..."])

def ghcr_pull_scope(url):
    """The ghcr.io token scope that pulls the repository a ghcr.io/v2 blob or manifest URL names."""
    prefix = "https://ghcr.io/v2/"
    if not url.startswith(prefix):
        fail("not a ghcr.io/v2 URL: {}".format(url))
    for kind in ("/blobs/", "/manifests/"):
        if kind in url:
            return "repository:{}:pull".format(url[len(prefix):url.index(kind)])
    fail("no blob or manifest in ghcr.io URL: {}".format(url))

def ghcr_headers(rctx, scope):
    """Download headers that carry an anonymous ghcr.io pull token for scope; ghcr.io answers 401 without one."""
    rctx.download(url = "https://ghcr.io/token?service=ghcr.io&scope=" + scope, output = "_ghcr_token.json")
    token = json.decode(rctx.read("_ghcr_token.json")).get("token")
    rctx.delete("_ghcr_token.json")
    if not token:
        fail("{}: ghcr.io gave no token for {}".format(rctx.name, scope))
    return {"Authorization": ["Bearer " + token]}
