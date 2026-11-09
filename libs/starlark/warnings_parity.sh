#!/bin/bash
# Fails when a compiled language neither maps warnings_as_errors nor opts out.
# Args: language=file[,file...] for a mapped language, language=- for an opt-out.
set -euo pipefail

status=0
for entry in "$@"; do
    lang="${entry%%=*}"
    files="${entry#*=}"
    if [[ "$files" == "-" ]]; then
        continue
    fi
    if ! grep -qE 'dx_warnings_as_errors\(|warnings = _[a-z_]*WARNINGS|warnings = _cgo_warnings' ${files//,/ }; then
        echo "$lang: no warnings_as_errors mapping in $files" >&2
        status=1
    fi
done
exit "$status"
