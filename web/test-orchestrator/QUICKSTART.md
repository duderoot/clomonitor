# TDD Orchestrator - Quick Start Guide

Get up and running with TDD testing in 5 minutes!

## Prerequisites

- Node.js 16+ and Yarn installed
- CLOMonitor web app code checked out

## Step 1: Setup (One-time)

```bash
cd web
./test-orchestrator/setup.sh
```

This will:
- Install all dependencies including Playwright
- Install Chromium browser
- Create report directories

## Step 2: Start Your App

```bash
# Terminal 1
cd web
yarn start
```

Wait for the app to be available at http://localhost:3000

## Step 3: Run Your First Test

```bash
# Terminal 2
cd web
yarn tdd-test dt-integration
```

You'll see:
- Browser window opens (not headless by default)
- Test steps execute with visual feedback
- Screenshots captured at each step
- Console output shows progress

## Understanding the Output

### Console Output

```
🔄 TDD Cycle 1: dt-integration-visibility
📝 Feature: Add DT visibility toggle to enable/disable DT data

✅ Step passed: Navigate to project detail page
❌ Step failed: Verify DT toggle exists
   Error: Timeout waiting for selector [data-testid="dt-visibility-toggle"]

📊 Cycle 1 Summary for "dt-integration-visibility"
Status: ❌ FAILED
```

### What This Means

The test is looking for a component with `data-testid="dt-visibility-toggle"` but it doesn't exist yet. This is **expected** in TDD - the test fails first!

## Step 4: Review Visual Feedback

### Open the HTML Report

```bash
open test-orchestrator/reports/dt-integration-cycle1-*.html
```

You'll see:
- ✅/❌ Status for each step
- Screenshots showing exactly what the browser sees
- Error messages and timing information

### Check Screenshots

```bash
ls test-orchestrator/reports/screenshots/dt-integration-*/
```

Each screenshot shows the page state during that test step.

## Step 5: Implement the Feature

Now that the test defines what we need, implement it:

### Option A: Manual Implementation

1. Open `web/src/layout/dt/index.tsx` (or create it)
2. Add the toggle component:

```tsx
<div data-testid="dt-visibility-toggle">
  <input
    type="checkbox"
    checked={showDT}
    onChange={(e) => setShowDT(e.target.checked)}
  />
  <label>Show DT Data</label>
</div>

{showDT && (
  <div data-testid="dt-data-section">
    {/* DT data here */}
  </div>
)}
```

### Option B: Use React Specialist Agent

Ask the agent:
> "Implement a DT visibility toggle component based on the test requirements in test-orchestrator/workflows/example-dt-integration.ts. The component should have data-testid attributes as specified in the tests."

## Step 6: Run the Test Again

```bash
yarn tdd-test dt-integration
```

Now more steps should pass! If not all steps pass, repeat the cycle:
1. Review the new screenshots
2. Identify what's missing
3. Implement the missing piece
4. Run tests again

## Step 7: Success! 🎉

When all tests pass:

```
✅ Step passed: Navigate to project detail page
✅ Step passed: Verify DT toggle exists
✅ Step passed: Toggle DT visibility ON
✅ Step passed: Verify DT data is displayed
✅ Step passed: Toggle DT visibility OFF
✅ Step passed: Verify DT data is hidden

🎉 Workflow "dt-integration-visibility" completed successfully in 2 cycle(s)!
```

## Creating Your Own Workflow

### 1. Define Test Steps

Create `test-orchestrator/workflows/my-feature.ts`:

```typescript
import { TDDWorkflow } from '../helpers/tdd-coordinator';
import { Page } from '@playwright/test';

export const myFeatureWorkflow: TDDWorkflow = {
  name: 'my-feature',
  description: 'My feature description',
  feature: 'What this feature does',

  testSteps: [
    {
      name: 'Test step 1',
      action: async (page: Page) => {
        // What to do
        await page.goto('/my-page');
      },
      validate: async (page: Page) => {
        // How to verify it worked
        return await page.isVisible('.my-element');
      },
      captureScreenshot: true
    }
  ],

  uiChangesRequired: [
    'List of changes needed to make this test pass'
  ]
};
```

### 2. Register It

Edit `test-orchestrator/cli.ts`:

```typescript
import { myFeatureWorkflow } from './workflows/my-feature';

const WORKFLOWS = {
  // ... existing workflows
  'my-feature': myFeatureWorkflow,
};
```

### 3. Run It

```bash
yarn tdd-test my-feature
```

## Common Commands

```bash
# Run a workflow
yarn tdd-test <workflow-name>

# Run in headless mode (no browser window)
yarn tdd-test <workflow-name> --headless true

# Run with custom cycles limit
yarn tdd-test <workflow-name> --cycles 3

# Run standard Playwright tests
yarn playwright

# Show Playwright UI
yarn playwright:ui

# Show last Playwright report
yarn playwright:report

# Get help
yarn tdd-test --help
```

## TDD Workflow Summary

```
1. Write failing test (RED)
   ↓
2. Run: yarn tdd-test <workflow>
   ↓
3. Review screenshots & errors
   ↓
4. Implement minimum code to pass
   ↓
5. Run: yarn tdd-test <workflow>
   ↓
6. All green? (GREEN)
   ↓
7. Refactor & improve
   ↓
8. Repeat for next feature
```

## Tips

1. **Keep tests small**: Test one thing at a time
2. **Use data-testid**: Makes selectors reliable
3. **Check screenshots**: They show exactly what happened
4. **Iterate quickly**: Small changes, frequent testing
5. **Read reports**: HTML reports have all the details

## Troubleshooting

### Port already in use
```bash
# Kill the process using port 3000
lsof -ti:3000 | xargs kill -9
```

### Playwright not found
```bash
yarn tdd-install
```

### Browser won't close
```bash
pkill -f chromium
```

## Next Steps

- Read the full [README.md](README.md)
- Explore example workflows in `workflows/`
- Create custom test helpers
- Integrate with CI/CD

---

**Happy TDD Testing!** 🚀

For questions or issues, check the main documentation or create an issue.
