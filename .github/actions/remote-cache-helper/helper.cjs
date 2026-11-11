"use strict";

const fs = require("fs");
const path = require("path");
const process = require("process");

function fail(message) {
  process.stdout.write(`::error::${message}\n`);
  process.exit(1);
}

const keyName = "x-buildbuddy-api-key";
const shell = `#!/bin/sh
key_name=${keyName}
h="\${REMOTE_CACHE_HEADER:-$key_name=$BUILDBUDDY_API_KEY}"
printf '{"headers":{"%s":["%s"]}}\\n' "\${h%%=*}" "\${h#*=}"
`;
const batch =
  `@if not defined REMOTE_CACHE_HEADER set "REMOTE_CACHE_HEADER=${keyName}=%BUILDBUDDY_API_KEY%"\r\n` +
  '@for /f "tokens=1* delims==" %%a in ("%REMOTE_CACHE_HEADER%") do @echo {"headers":{"%%a":["%%b"]}}\r\n';

let args = process.env.DX_REMOTE_CACHE_ARG || "";
const configured = process.env.REMOTE_CACHE_HEADER || "";
const key = process.env.BUILDBUDDY_API_KEY || "";
if (configured !== "" || key !== "") {
  const header = configured !== "" ? configured : `${keyName}=${key}`;
  const at = header.indexOf("=");
  const name = at < 0 ? header : header.slice(0, at);
  const value = at < 0 ? header : header.slice(at + 1);
  if (at < 0 || name === "" || value === "") {
    fail("REMOTE_CACHE_HEADER must look like name=value");
  }
  if (/["\\%&|<>^]/.test(name + value)) {
    fail(
      'The remote cache header has a character the credential helper cannot carry: one of " \\ % & | < > ^',
    );
  }
  let host = "remote.buildbuddy.io";
  if (configured !== "") {
    host = args.replace(/^--remote_cache=/, "");
    host = host.replace(/^.*?:\/\//, "");
    host = host.split("/")[0];
    host = host.slice(host.lastIndexOf("@") + 1);
    host = host.split(":")[0] || "remote.buildbuddy.io";
  }
  const dir = path.join(process.env.RUNNER_TEMP, "dx-credential-helper");
  fs.mkdirSync(dir, { recursive: true });
  const windows = process.env.RUNNER_OS === "Windows";
  let helper = path.join(dir, windows ? "helper.bat" : "helper.sh");
  fs.writeFileSync(helper, windows ? batch : shell, { mode: 0o700 });
  fs.chmodSync(helper, 0o700);
  if (windows) {
    helper = helper.replace(/\\/g, "/");
  }
  args = `${args === "" ? "" : `${args} `}--credential_helper=${host}=${helper}`;
}
if (args !== "") {
  fs.appendFileSync(process.env.GITHUB_ENV, `BB_ARGS=${args}\n`);
}
