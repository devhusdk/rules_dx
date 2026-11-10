#!/bin/bash
# ocamlopt calls the archiver as ar does ("rc out objects"); libtool wants "-static -o out objects".
shift
out="$1"
shift
exec "$DX_OCAML_LIBTOOL" -static -o "$out" "$@"
