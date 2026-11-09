"""Prints the manifest digest to pin a Bicep registry module.

Run through the bicep_pin target: bazel run //bicep/tools:pin -- br:mcr.microsoft.com/bicep/avm/res/storage/storage-account:0.14.0
It needs network access to the registry and uses anonymous access.
"""

import hashlib
import json
import re
import sys
import urllib.error
import urllib.request

ACCEPT = ", ".join([
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
])


def parse_ref(ref):
    """Splits br:HOST/PATH:TAG into (host, path, tag)."""
    if not ref.startswith("br:"):
        raise ValueError("ref %r must start with br:" % ref)
    name, _, tag = ref[3:].rpartition(":")
    host, _, path = name.partition("/")
    if not tag or "/" in tag or not host or not path:
        raise ValueError("ref %r must look like br:HOST/PATH:TAG" % ref)
    return host, path, tag


def digest_of(manifest_bytes):
    return "sha256:" + hashlib.sha256(manifest_bytes).hexdigest()


def _get(url, headers):
    request = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def fetch_manifest(host, path, tag):
    url = "https://%s/v2/%s/manifests/%s" % (host, path, tag)
    try:
        return _get(url, {"Accept": ACCEPT})
    except urllib.error.HTTPError as error:
        challenge = error.headers.get("WWW-Authenticate", "")
        if error.code != 401 or not challenge.startswith("Bearer "):
            raise
    params = dict(re.findall(r'(\w+)="([^"]*)"', challenge))
    token_url = "%s?service=%s&scope=repository:%s:pull" % (params["realm"], params.get("service", ""), path)
    token = json.loads(_get(token_url, {}))
    bearer = token.get("token") or token.get("access_token")
    return _get(url, {"Accept": ACCEPT, "Authorization": "Bearer " + bearer})


def main(argv):
    if len(argv) != 1:
        sys.exit("usage: pin br:HOST/PATH:TAG")
    host, path, tag = parse_ref(argv[0])
    digest = digest_of(fetch_manifest(host, path, tag))
    print('bicep.registry_module(name = "...", ref = "%s", digest = "%s")' % (argv[0], digest))


if __name__ == "__main__":
    main(sys.argv[1:])
