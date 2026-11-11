"""Line coverage for Python tests under `bazel coverage`: writes LCOV to COVERAGE_OUTPUT_FILE."""

import atexit
import os
import sys
import threading


def _manifest(path):
    with open(path, encoding="utf-8") as handle:
        return [line.strip().replace("\\", "/") for line in handle if line.strip().endswith(".py")]


def _executable_lines(path):
    with open(path, "rb") as handle:
        top = compile(handle.read(), path, "exec")
    lines = set()
    pending = [top]
    while pending:
        code = pending.pop()
        lines.update(line for _, _, line in code.co_lines() if line)
        pending.extend(const for const in code.co_consts if hasattr(const, "co_lines"))
    return lines


def _install():
    manifest = os.environ.get("COVERAGE_MANIFEST")
    output = os.environ.get("COVERAGE_OUTPUT_FILE")
    if not manifest or not output or not os.path.isfile(manifest):
        return
    sources = _manifest(manifest)
    if not sources:
        return
    hits = {}
    decided = {}

    def owner(filename):
        known = decided.get(filename)
        if known is None:
            normal = filename.replace("\\", "/")
            known = next((s for s in sources if normal.endswith("/" + s) or normal == s), "")
            decided[filename] = known
        return known

    def local(frame, event, _arg):
        if event == "line":
            counts = hits[frame.f_code.co_filename]
            counts[frame.f_lineno] = counts.get(frame.f_lineno, 0) + 1
        return local

    def tracer(frame, _event, _arg):
        filename = frame.f_code.co_filename
        if not filename.endswith(".py") or not owner(filename):
            return None
        counts = hits.setdefault(filename, {})
        counts[frame.f_lineno] = counts.get(frame.f_lineno, 0) + 1
        return local

    def write():
        sys.settrace(None)
        threading.settrace(None)
        records = []
        for filename, counts in sorted(hits.items()):
            records.append("SF:" + owner(filename))
            for line in sorted(_executable_lines(filename)):
                records.append("DA:%d,%d" % (line, counts.get(line, 0)))
            records.append("end_of_record")
        with open(output, "a", encoding="utf-8") as handle:
            handle.write("\n".join(records) + "\n")

    atexit.register(write)
    threading.settrace(tracer)
    sys.settrace(tracer)


_install()
