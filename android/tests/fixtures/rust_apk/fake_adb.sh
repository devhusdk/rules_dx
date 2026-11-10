#!/bin/sh
echo "$@" >> "$ADB_CAPTURE"
exit "${ADB_EXIT:-0}"
