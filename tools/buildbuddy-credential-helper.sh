#!/bin/sh
printf '{"headers":{"x-buildbuddy-api-key":["%s"]}}\n' "$BUILDBUDDY_API_KEY"
