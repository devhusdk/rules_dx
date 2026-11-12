"""Public TypeScript module extensions."""

load("//typescript/private:extensions.bzl", _typescript = "typescript")
load("//typescript/private/playwright:extensions.bzl", _playwright = "playwright")

typescript_playwright = _playwright
typescript = _typescript
