#!/bin/sh
key_name=x-buildbuddy-api-key
h="${REMOTE_CACHE_HEADER:-$key_name=$BUILDBUDDY_API_KEY}"
printf '{"headers":{"%s":["%s"]}}\n' "${h%%=*}" "${h#*=}"
