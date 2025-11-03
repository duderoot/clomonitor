import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for TDD Orchestrator
 * See https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: './workflows',

  // Maximum time one test can run
  timeout: 60 * 1000,

  // Test execution settings
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1, // Run tests sequentially for TDD workflow

  // Reporter configuration
  reporter: [
    ['html', { outputFolder: 'reports/playwright-html' }],
    ['json', { outputFile: 'reports/test-results.json' }],
    ['list']
  ],

  // Shared settings for all tests
  use: {
    // Base URL for navigation
    baseURL: process.env.BASE_URL || 'http://localhost:3000',

    // Collect trace on failure
    trace: 'on-first-retry',

    // Screenshot on failure
    screenshot: 'on',

    // Video on failure
    video: 'retain-on-failure',

    // Browser context options
    viewport: { width: 1920, height: 1080 },

    // Ignore HTTPS errors
    ignoreHTTPSErrors: true,

    // Timeout for actions
    actionTimeout: 10000,

    // Timeout for navigation
    navigationTimeout: 30000,
  },

  // Configure projects for major browsers
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },

    // Uncomment to test in other browsers
    // {
    //   name: 'firefox',
    //   use: { ...devices['Desktop Firefox'] },
    // },
    // {
    //   name: 'webkit',
    //   use: { ...devices['Desktop Safari'] },
    // },
  ],

  // Web server configuration (optional - runs dev server automatically)
  // webServer: {
  //   command: 'yarn start',
  //   url: 'http://localhost:3000',
  //   reuseExistingServer: !process.env.CI,
  //   timeout: 120 * 1000,
  // },
});
