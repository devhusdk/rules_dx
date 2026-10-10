#!/bin/sh
slot="${DX_FLAKY_SLOT:-manual}"
marker="/tmp/dx_flaky_once_${slot}"
if [ ! -e "$marker" ]; then
  touch "$marker"
  echo "fails_once[${slot}]: first attempt fails by design" >&2
  exit 1
fi
rm -f "$marker"
echo "fails_once[${slot}]: retry attempt passes"
exit 0
