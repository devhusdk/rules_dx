#!/bin/sh
head='<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="cases">'
tail='</testsuite></testsuites>'
case "${DX_CASES:-pass}" in
  none)
    printf '%s%s\n' "$head" "$tail" > "$XML_OUTPUT_FILE"
    exit 0
    ;;
  fail)
    printf '%s%s%s%s\n' "$head" '<testcase name="passes" classname="cases"/>' \
      '<testcase name="fails" classname="cases"><failure message="expected 1, got 2">assertion trace</failure></testcase>' \
      "$tail" > "$XML_OUTPUT_FILE"
    exit 1
    ;;
  *)
    printf '%s%s%s%s\n' "$head" '<testcase name="passes" classname="cases"/>' \
      '<testcase name="ignored" classname="cases"><skipped message="ignored by design"/></testcase>' \
      "$tail" > "$XML_OUTPUT_FILE"
    exit 0
    ;;
esac
