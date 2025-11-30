# TDD Orchestrator - Architecture Overview

## System Design

The TDD Orchestrator is a custom script-based system that coordinates Test-Driven Development workflows using Playwright for browser automation and test execution.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Interface                           │
│                        (cli.ts)                                 │
│                                                                 │
│  • Parse arguments                                              │
│  • Select workflow                                              │
│  • Initialize coordinator                                       │
│  • Display results                                              │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                     TDD Coordinator                             │
│                   (tdd-coordinator.ts)                          │
│                                                                 │
│  • Manages browser lifecycle                                    │
│  • Executes test cycles                                         │
│  • Tracks cycle history                                         │
│  • Generates session reports                                    │
│  • Provides feedback loop                                       │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Test Runner                                 │
│                   (test-runner.ts)                              │
│                                                                 │
│  • Executes test steps sequentially                             │
│  • Captures screenshots                                         │
│  • Validates outcomes                                           │
│  • Generates HTML reports                                       │
│  • Provides helper methods                                      │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Workflow Definitions                         │
│              (workflows/*.ts)                                   │
│                                                                 │
│  • Define test steps                                            │
│  • Specify actions                                              │
│  • Define validations                                           │
│  • Document UI requirements                                     │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Playwright                                 │
│                                                                 │
│  • Browser automation                                           │
│  • Page interactions                                            │
│  • Screenshot capture                                           │
│  • Video recording                                              │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. CLI Interface (`cli.ts`)

**Responsibility**: Entry point for user interaction

**Features**:
- Command-line argument parsing
- Workflow selection and validation
- Help documentation display
- Process lifecycle management
- Error handling and exit codes

**Key Functions**:
```typescript
parseArgs()         // Parse CLI arguments
printHelp()         // Display help information
listWorkflows()     // Show available workflows
main()              // Main execution flow
```

### 2. TDD Coordinator (`tdd-coordinator.ts`)

**Responsibility**: Orchestrate TDD cycles

**Features**:
- Browser initialization and cleanup
- Cycle execution management
- History tracking
- Feedback generation
- Multi-cycle coordination

**Key Methods**:
```typescript
initialize()                    // Setup browser
runTDDCycle(workflow)          // Execute single cycle
runMultipleCycles()            // Execute multiple cycles
generateNextActions()          // Create feedback
generateFullReport()           // Create session report
cleanup()                      // Teardown browser
```

**State Management**:
- `browser`: Chromium instance
- `context`: Browser context with recording
- `page`: Current page instance
- `cycleHistory`: Array of cycle results

### 3. Test Runner (`test-runner.ts`)

**Responsibility**: Execute individual test workflows

**Features**:
- Step-by-step test execution
- Screenshot management
- Error handling
- Report generation
- Helper utilities

**Key Methods**:
```typescript
runSteps(page, steps)          // Execute test steps
captureScreenshot(page, name)  // Take screenshot
generateHTMLReport(report)     // Create HTML report
navigateAndWait()              // Navigate helper
waitForElement()               // Wait helper
clickAndWait()                 // Click helper
typeAndWait()                  // Type helper
```

**Screenshot Strategy**:
- Auto-numbering: `000-step-name.png`
- Full-page capture
- Error screenshots: `step-name-ERROR.png`
- Organized by test run

### 4. Workflow Definitions (`workflows/*.ts`)

**Responsibility**: Define test scenarios

**Structure**:
```typescript
interface TDDWorkflow {
  name: string;              // Workflow identifier
  description: string;       // Short description
  feature: string;           // Feature being tested
  testSteps: TestStep[];     // Array of test steps
  uiChangesRequired?: string[];  // Implementation hints
}

interface TestStep {
  name: string;              // Step description
  action: (page) => Promise<void>;    // What to do
  validate?: (page) => Promise<bool>; // How to verify
  captureScreenshot?: boolean;        // Take screenshot?
}
```

## Data Flow

### Test Execution Flow

```
User runs CLI command
        ↓
CLI parses arguments and selects workflow
        ↓
TDD Coordinator initializes browser
        ↓
For each cycle:
    ↓
    Test Runner receives workflow
        ↓
    For each test step:
        ↓
        Execute action on page
        ↓
        Run validation (if defined)
        ↓
        Capture screenshot (if needed/failed)
        ↓
        Store result
        ↓
    Generate HTML report
    ↓
    Return cycle result
    ↓
Coordinator analyzes results
    ↓
Generate feedback (if failed)
    ↓
Display summary to user
    ↓
Cleanup browser
```

### Report Generation Flow

```
Test step completes
        ↓
Result object created:
  - stepName
  - passed (bool)
  - screenshot path
  - error message
  - timestamp
  - duration
        ↓
All steps complete
        ↓
Test report created:
  - testName
  - passed (bool)
  - steps array
  - timing info
  - screenshot dir
        ↓
HTML generation:
  - Inline CSS
  - Embedded images (relative paths)
  - Interactive elements
  - Step-by-step breakdown
        ↓
Save to reports/ directory
        ↓
Return report path
```

## Integration Points

### With UI Development Agents

The orchestrator is designed to work with specialized agents:

```
┌──────────────┐         ┌──────────────┐
│   TDD Test   │────────▶│  React Agent │
│ Orchestrator │         │  (UI Dev)    │
└──────┬───────┘         └──────┬───────┘
       │                        │
       │ 1. Run test           │
       │    (fails)            │
       │                        │
       │ 2. Show               │
       │    requirements       │
       │                        │
       │ 3. Agent implements   │
       │    feature            │◀─────┐
       │                        │      │
       │ 4. Run test           │      │
       │    (check)            │      │
       │                        │      │
       └────────────────────────┘      │
                                        │
                                   Iterate
                                   until pass
```

### With Playwright MCP Server

The orchestrator uses Playwright's API but can also leverage MCP tools:

```typescript
// Direct Playwright usage (current)
await page.click(selector);
await page.screenshot(options);

// Can be extended with MCP tools
// For additional browser control
```

### With Project Codebase

```
web/
├── src/                    # React application
│   └── layout/
│       └── dt/             # DT components tested
├── test-orchestrator/      # This system
│   ├── workflows/          # Test definitions
│   └── reports/            # Generated artifacts
└── package.json            # Shared dependencies
```

## Configuration

### Playwright Config (`playwright.config.ts`)

Controls:
- Test timeout (60s)
- Parallelization (sequential for TDD)
- Retry strategy (0 retries for TDD)
- Screenshot/video settings
- Browser selection (Chromium by default)
- Base URL configuration

### TypeScript Config (`tsconfig.json`)

Settings:
- Extends parent config
- Node.js module resolution
- CommonJS modules for ts-node
- Strict type checking
- Playwright types included

## Extension Points

### Adding New Workflows

1. Create workflow file in `workflows/`
2. Export workflow object
3. Register in `cli.ts`
4. No code changes to core needed

### Custom Test Helpers

```typescript
// Extend PlaywrightTestRunner
class MyCustomRunner extends PlaywrightTestRunner {
  async customHelper(page: Page, ...args) {
    // Custom logic
    await this.captureScreenshot(page, 'custom-step');
  }
}
```

### Custom Reporters

```typescript
// Implement custom report format
class JSONReporter {
  generate(report: TestReport): string {
    return JSON.stringify(report, null, 2);
  }
}
```

### Integration with CI/CD

```yaml
# GitHub Actions example
- name: Run TDD Tests
  run: |
    yarn start &
    sleep 10
    yarn tdd-test dt-integration --headless true

- name: Upload Reports
  uses: actions/upload-artifact@v3
  with:
    name: tdd-reports
    path: web/test-orchestrator/reports/
```

## Performance Considerations

### Browser Management

- Single browser instance per session
- Context reuse across cycles
- Proper cleanup on exit
- Video recording only on failure

### Screenshot Optimization

- Full-page screenshots (comprehensive but slower)
- Conditional capture (only on error or when specified)
- Sequential numbering for easy reference
- Organized directory structure

### Report Generation

- HTML with inline styles (single file)
- Relative image paths (portable)
- JSON summary (lightweight)
- Lazy video encoding

## Security Considerations

### Credential Handling

- No credentials stored in workflows
- Environment variables for sensitive data
- GitHub tokens via environment only

### File System

- Reports in dedicated directory
- .gitignore for artifacts
- No arbitrary file writes
- Sandboxed execution

## Monitoring and Debugging

### Console Logging

```
Browser Console → Forwarded to CLI
Page Errors     → Captured and logged
Network Events  → Available for inspection
```

### Debug Mode

```bash
# Enable Playwright debug
DEBUG=pw:api yarn tdd-test <workflow>

# Slow motion execution
# Edit tdd-coordinator.ts slowMo value
```

### Visual Debugging

- Browser visible by default (`headless: false`)
- Step-by-step screenshots
- Video recording on failure
- Interactive HTML reports

## Future Enhancements

Potential improvements:

1. **Parallel execution** for independent test steps
2. **Custom assertions library** for common validations
3. **Screenshot diffing** for visual regression
4. **Performance metrics** collection
5. **Integration with MCP Server** for enhanced browser control
6. **Mobile device emulation** workflows
7. **A11y testing** integration
8. **Network mocking** capabilities
9. **Database state setup** helpers
10. **Multi-browser testing** support

## Best Practices

### Workflow Design

- Keep steps atomic and independent
- Use descriptive step names
- Add validations for critical steps
- Document UI requirements clearly
- Use data-testid for stable selectors

### Test Maintenance

- Review and update workflows regularly
- Keep workflows in version control
- Document breaking changes
- Use semantic versioning for workflows

### Performance

- Minimize unnecessary screenshots
- Use targeted selectors
- Optimize wait strategies
- Clean up resources properly

## Troubleshooting Guide

### Common Issues

1. **Browser won't start**: Check Playwright installation
2. **Timeouts**: Increase timeout in config
3. **Screenshots missing**: Check directory permissions
4. **Reports not generated**: Check disk space

### Debug Checklist

- [ ] Dev server running on correct port?
- [ ] Playwright browsers installed?
- [ ] Sufficient disk space for reports?
- [ ] Correct base URL configured?
- [ ] Network connectivity working?
- [ ] Selectors in workflow correct?

---

This architecture supports a robust TDD workflow while remaining flexible and extensible for future needs.
