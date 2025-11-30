# TDD Orchestrator with Playwright

A powerful TDD (Test-Driven Development) workflow automation tool that coordinates between UI development and end-to-end testing using Playwright.

## 🎯 Purpose

This orchestrator enables a seamless TDD workflow where:

1. **Tests are written first** (Red phase) - Define expected behavior through Playwright tests
2. **UI is implemented** to make tests pass (Green phase)
3. **Code is refined** while maintaining test coverage (Refactor phase)

The orchestrator automatically:
- Runs test workflows with visual feedback
- Captures screenshots at each step
- Records videos of test execution
- Generates detailed HTML reports
- Provides actionable feedback for UI implementation

## 📁 Structure

```
test-orchestrator/
├── cli.ts                          # Main CLI entry point
├── helpers/
│   ├── test-runner.ts             # Core test execution engine
│   └── tdd-coordinator.ts         # TDD workflow coordinator
├── workflows/
│   └── example-dt-integration.ts  # Example workflow definitions
├── reports/                        # Generated test reports
│   ├── screenshots/               # Step-by-step screenshots
│   ├── videos/                    # Test execution videos
│   └── *.html                     # HTML test reports
├── playwright.config.ts           # Playwright configuration
├── tsconfig.json                  # TypeScript configuration
└── README.md                      # This file
```

## 🚀 Quick Start

### 1. Install Dependencies

```bash
cd web
yarn install
yarn tdd-install  # Installs Playwright browsers
```

### 2. Start Your Development Server

The orchestrator needs your app running to test it:

```bash
# Terminal 1: Start the app
yarn start
```

### 3. Run a TDD Workflow

```bash
# Terminal 2: Run TDD test
yarn tdd-test dt-integration
```

## 📚 Available Workflows

### DT Integration Workflow

Tests the DT (Dependency Track) visibility toggle feature:

```bash
yarn tdd-test dt-integration
```

**What it tests:**
- Navigation to project detail page
- DT toggle component existence
- Toggle ON/OFF functionality
- Conditional rendering of DT data

### Project Search Workflow

Tests project search and filtering:

```bash
yarn tdd-test project-search
```

**What it tests:**
- Search input functionality
- Result filtering
- Clear search behavior

## 🔧 CLI Usage

### Basic Command

```bash
yarn tdd-test <workflow-name> [options]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--cycles <n>` | Maximum number of test cycles | 5 |
| `--base-url <url>` | Base URL of your app | http://localhost:3000 |
| `--headless <bool>` | Run browser in headless mode | false |
| `--help, -h` | Show help message | - |

### Examples

```bash
# Run with default settings (browser visible)
yarn tdd-test dt-integration

# Run in headless mode
yarn tdd-test dt-integration --headless true

# Custom base URL and cycles
yarn tdd-test project-search --base-url http://localhost:8080 --cycles 3
```

## 📝 Creating Custom Workflows

### 1. Define Your Workflow

Create a new file in `workflows/` directory:

```typescript
// workflows/my-feature.ts
import { TDDWorkflow } from '../helpers/tdd-coordinator';
import { Page } from '@playwright/test';

export const myFeatureWorkflow: TDDWorkflow = {
  name: 'my-feature',
  description: 'Test my awesome feature',
  feature: 'Feature description',

  testSteps: [
    {
      name: 'Step 1: Navigate to page',
      action: async (page: Page) => {
        await page.goto('/my-page');
      },
      validate: async (page: Page) => {
        return await page.isVisible('h1');
      },
      captureScreenshot: true
    },

    {
      name: 'Step 2: Click button',
      action: async (page: Page) => {
        await page.click('[data-testid="my-button"]');
      },
      validate: async (page: Page) => {
        const result = await page.$('[data-testid="result"]');
        return result !== null;
      },
      captureScreenshot: true
    }
  ],

  uiChangesRequired: [
    'Add button with data-testid="my-button"',
    'Implement click handler',
    'Display result element'
  ]
};
```

### 2. Register the Workflow

Update `cli.ts` to include your workflow:

```typescript
import { myFeatureWorkflow } from './workflows/my-feature';

const WORKFLOWS = {
  'dt-integration': dtIntegrationWorkflow,
  'project-search': projectSearchWorkflow,
  'my-feature': myFeatureWorkflow,  // Add here
};
```

### 3. Run Your Workflow

```bash
yarn tdd-test my-feature
```

## 🎨 TDD Workflow Pattern

### Red-Green-Refactor Cycle

```
1. Write Test (RED)
   ↓
2. Run test → It fails ❌
   ↓
3. Review screenshots & errors
   ↓
4. Implement UI changes
   ↓
5. Run test → It passes ✅
   ↓
6. Refactor if needed
   ↓
7. Repeat for next feature
```

### Example Session

```bash
# Cycle 1: Write the test first
yarn tdd-test dt-integration

# Output:
# ❌ Step failed: Verify DT toggle exists
# Error: Timeout waiting for selector [data-testid="dt-visibility-toggle"]
# 📸 Screenshots: test-orchestrator/reports/screenshots/...

# Now implement the UI component...
# Add the toggle component with proper data-testid

# Cycle 2: Run again after implementation
yarn tdd-test dt-integration

# Output:
# ✅ Step passed: Navigate to project detail page
# ✅ Step passed: Verify DT toggle exists
# ✅ Step passed: Toggle DT visibility ON
# 🎉 Workflow completed successfully!
```

## 📊 Reports and Artifacts

### HTML Reports

Interactive HTML reports with screenshots for each test step:

```
test-orchestrator/reports/dt-integration-cycle1-2024-10-04T*.html
```

Open in browser to review:
- Step-by-step execution
- Pass/fail status
- Screenshots
- Error messages
- Timing information

### Screenshots

Full-page screenshots captured at each step:

```
test-orchestrator/reports/screenshots/dt-integration-*/
├── 000-Navigate-to-project-detail-page.png
├── 001-Verify-DT-toggle-exists.png
├── 002-Toggle-DT-visibility-ON.png
└── ...
```

### Videos

Test execution videos (retained on failure):

```
test-orchestrator/reports/videos/
```

### JSON Summary

Session summary in JSON format:

```json
{
  "sessionStart": "2024-10-04T...",
  "sessionEnd": "2024-10-04T...",
  "totalCycles": 2,
  "workflows": ["dt-integration"],
  "cycles": [
    {
      "workflow": "dt-integration",
      "cycle": 1,
      "passed": false,
      "duration": 5234,
      "failedSteps": ["Verify DT toggle exists"],
      "screenshotDir": "..."
    }
  ]
}
```

## 🔗 Integration with React/Vue/Angular Agents

This orchestrator is designed to work seamlessly with specialized UI development agents:

### Workflow with React Agent

```bash
# 1. Define your test workflow (already done)
# 2. Run the test to see what's missing
yarn tdd-test dt-integration

# 3. Ask React specialist agent to implement the feature:
# "Implement a DT visibility toggle component based on the
#  test requirements in test-orchestrator/workflows/example-dt-integration.ts"

# 4. Run test again to verify
yarn tdd-test dt-integration
```

### Best Practices

1. **Write Tests First**: Define expected behavior before implementation
2. **Use data-testid**: Make components easily testable
   ```tsx
   <button data-testid="dt-visibility-toggle">Toggle DT</button>
   ```

3. **Small Steps**: Break features into small, testable steps
4. **Review Screenshots**: Visual feedback helps identify UI issues
5. **Iterate**: Use the cycle history to track progress

## 🛠️ Advanced Usage

### Programmatic API

Use the orchestrator programmatically:

```typescript
import { TDDCoordinator } from './test-orchestrator/helpers/tdd-coordinator';
import { myWorkflow } from './test-orchestrator/workflows/my-workflow';

const coordinator = new TDDCoordinator('http://localhost:3000', false);

await coordinator.initialize();
const result = await coordinator.runTDDCycle(myWorkflow);
await coordinator.cleanup();

if (result.passed) {
  console.log('✅ All tests passed!');
} else {
  console.log('❌ Tests failed:', result.nextActions);
}
```

### Custom Test Helpers

The `PlaywrightTestRunner` class provides helper methods:

```typescript
const runner = new PlaywrightTestRunner('my-test');

// Navigate with wait
await runner.navigateAndWait(page, '/my-route');

// Wait for element
await runner.waitForElement(page, '.my-selector');

// Click and wait for network idle
await runner.clickAndWait(page, 'button');

// Type with debounce
await runner.typeAndWait(page, 'input', 'search query');

// Capture screenshot
await runner.captureScreenshot(page, 'custom-step');
```

## 🐛 Debugging

### Enable Debug Mode

Set Playwright debug environment variable:

```bash
DEBUG=pw:api yarn tdd-test dt-integration
```

### Slow Motion

Run with slow motion for better visibility:

```typescript
// In tdd-coordinator.ts, adjust slowMo value
this.browser = await chromium.launch({
  headless: false,
  slowMo: 500  // Increase for slower execution
});
```

### Inspect Element Selectors

Use Playwright Inspector:

```bash
yarn playwright:ui
```

## 📋 Checklist for New Features

- [ ] Create workflow file in `workflows/`
- [ ] Define test steps with clear names
- [ ] Add validation logic for each step
- [ ] Enable screenshots for critical steps
- [ ] List required UI changes in `uiChangesRequired`
- [ ] Register workflow in `cli.ts`
- [ ] Run test to verify it fails correctly (Red)
- [ ] Implement UI changes
- [ ] Run test to verify it passes (Green)
- [ ] Refactor and optimize

## 🔍 Troubleshooting

### "Cannot find module '@playwright/test'"

Run:
```bash
yarn install
yarn tdd-install
```

### "Connection refused" errors

Ensure your dev server is running:
```bash
yarn start
```

### Tests timeout

Increase timeout in `playwright.config.ts`:
```typescript
timeout: 120 * 1000,  // 2 minutes
```

### Browser doesn't close

Force cleanup:
```bash
pkill -f chromium
```

## 📚 Resources

- [Playwright Documentation](https://playwright.dev)
- [TDD Best Practices](https://martinfowler.com/bliki/TestDrivenDevelopment.html)
- [Page Object Model](https://playwright.dev/docs/pom)

## 🤝 Contributing

To add new workflows or improve the orchestrator:

1. Create workflow file in `workflows/`
2. Update `cli.ts` to register it
3. Test the workflow end-to-end
4. Document the workflow in this README

## 📝 License

This orchestrator is part of the CLOMonitor project and follows the same license.
