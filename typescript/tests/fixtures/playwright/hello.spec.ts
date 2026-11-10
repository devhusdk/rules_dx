import { expect, test } from "@playwright/test";

test("serves the page", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("h1")).toHaveText("Hello from rules_dx");
});
