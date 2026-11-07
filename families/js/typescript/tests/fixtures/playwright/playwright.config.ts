import { defineConfig } from "@playwright/test";

const port = Number(process.env.DX_PLAYWRIGHT_PORT);

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  use: { baseURL: `http://127.0.0.1:${port}`, screenshot: "only-on-failure" },
  webServer: { command: process.env.DX_PLAYWRIGHT_APP ?? "", port },
});
