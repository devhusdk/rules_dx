"""check-jsonschema entry point.

dx runs the tool offline. `--dx-ref-map FILE` names a JSON object that maps every
http(s) URI a schema may reference to the checked-in copy that stands in for it. The
tool's reference retrieval is replaced so mapped URIs are read from disk and every
other http(s) URI fails, and `requests` refuses to connect at all.
"""

import importlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

REF_MAP_FLAG = "--dx-ref-map"


def pop_ref_map(argv):
    """Removes `--dx-ref-map FILE` (or `=FILE`) from argv and returns the map path."""
    path = None
    rest = []
    items = iter(argv)
    for item in items:
        if item == REF_MAP_FLAG:
            path = next(items, None)
            if path is None:
                sys.exit("check_jsonschema: " + REF_MAP_FLAG + " needs a file")
        elif item.startswith(REF_MAP_FLAG + "="):
            path = item.split("=", 1)[1]
        else:
            rest.append(item)
    argv[:] = rest
    return path


def load_ref_map(path):
    """Reads the URI to file map."""
    with open(path, encoding="utf-8") as handle:
        mapping = json.load(handle)
    if not isinstance(mapping, dict):
        sys.exit("check_jsonschema: " + path + " must hold a JSON object")
    return {str(uri): str(target) for uri, target in mapping.items()}


def install_offline_retrieval(mapping):
    """Serves mapped URIs from disk and refuses every other http(s) URI."""
    import urllib.parse

    referencing = importlib.import_module("referencing")
    exceptions = importlib.import_module("referencing.exceptions")
    draft = importlib.import_module("referencing.jsonschema").DRAFT202012
    resolver = importlib.import_module("check_jsonschema.schema_loader.resolver")
    original = resolver.create_retrieve_callable

    def create_retrieve_callable(
        parser_set, retrieval_uri, id_attribute, disable_cache
    ):
        fallback = original(parser_set, retrieval_uri, id_attribute, disable_cache)
        base = id_attribute if id_attribute is not None else retrieval_uri

        def retrieve(uri):
            full = uri
            if base is not None and urllib.parse.urlsplit(uri).scheme == "":
                full = urllib.parse.urljoin(base, uri)
            key = full.split("#", 1)[0]
            if key in mapping:
                with open(mapping[key], encoding="utf-8") as handle:
                    return referencing.Resource.from_contents(
                        json.load(handle), default_specification=draft
                    )
            if urllib.parse.urlsplit(full).scheme in ("http", "https"):
                raise exceptions.Unretrievable(full)
            return fallback(uri)

        return retrieve

    resolver.create_retrieve_callable = create_retrieve_callable
    block_network()


def block_network():
    """Makes `requests` refuse to connect."""
    requests = importlib.import_module("requests")
    adapters = importlib.import_module("requests.adapters")

    def refuse(self, request, *args, **kwargs):
        raise requests.exceptions.ConnectionError(
            "dx runs check-jsonschema offline: " + str(request.url)
        )

    adapters.HTTPAdapter.send = refuse


def main():
    """Runs the tool with offline reference retrieval."""
    map_path = pop_ref_map(sys.argv)
    tool = import_tool("check_jsonschema", "main")
    if map_path is not None:
        install_offline_retrieval(load_ref_map(map_path))
    return tool()


if __name__ == "__main__":
    sys.exit(main())
