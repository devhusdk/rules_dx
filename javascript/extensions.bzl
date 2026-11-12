"""Public JavaScript module extensions."""

load("//javascript/private:dev_hub.bzl", _dev_hub = "dx_dev_hub")
load("@aspect_rules_js//npm:extensions.bzl", _npm = "npm", _pnpm = "pnpm")

javascript_dev_hub = _dev_hub
npm = _npm
pnpm = _pnpm
