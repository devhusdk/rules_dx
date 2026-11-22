import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  use: { baseURL: process.env.DX_PLAYWRIGHT_URL, screenshot: "only-on-failure" },
  webServer: {
    command: process.env.DX_PLAYWRIGHT_APP ?? "",
    wait: { stdout: /listening on (?<dx_playwright_url>http:\/\/\S+)/ },
  },
});
