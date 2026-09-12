import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests",
  outputDir: "test-results/playwright",
  timeout: 30000,
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 1440, height: 1000 },
    trace: "retain-on-failure",
  },
  webServer: {
    command: "python3 ../scripts/ui_server.py",
    url: "http://127.0.0.1:4173/api/health",
    reuseExistingServer: false,
    timeout: 120000,
  },
});
