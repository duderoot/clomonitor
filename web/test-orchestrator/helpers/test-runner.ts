import { Page, Browser, BrowserContext } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

export interface TestStep {
  name: string;
  action: (page: Page) => Promise<void>;
  validate?: (page: Page) => Promise<boolean>;
  captureScreenshot?: boolean;
}

export interface TestResult {
  stepName: string;
  passed: boolean;
  screenshot?: string;
  error?: string;
  timestamp: Date;
  duration: number;
}

export interface TestReport {
  testName: string;
  passed: boolean;
  steps: TestResult[];
  startTime: Date;
  endTime: Date;
  totalDuration: number;
  screenshotDir: string;
}

export class PlaywrightTestRunner {
  private screenshotCounter = 0;
  private screenshotDir: string;

  constructor(
    private testName: string,
    private baseUrl: string = 'http://localhost:3000'
  ) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    this.screenshotDir = path.join(
      process.cwd(),
      'test-orchestrator',
      'reports',
      'screenshots',
      `${this.testName}-${timestamp}`
    );

    // Create screenshot directory
    fs.mkdirSync(this.screenshotDir, { recursive: true });
  }

  async captureScreenshot(page: Page, stepName: string): Promise<string> {
    const screenshotPath = path.join(
      this.screenshotDir,
      `${String(this.screenshotCounter).padStart(3, '0')}-${stepName.replace(/[^a-zA-Z0-9]/g, '-')}.png`
    );

    await page.screenshot({
      path: screenshotPath,
      fullPage: true
    });

    this.screenshotCounter++;
    return screenshotPath;
  }

  async runSteps(
    page: Page,
    steps: TestStep[]
  ): Promise<TestReport> {
    const startTime = new Date();
    const results: TestResult[] = [];
    let allPassed = true;

    for (const step of steps) {
      const stepStartTime = Date.now();
      const result: TestResult = {
        stepName: step.name,
        passed: false,
        timestamp: new Date(),
        duration: 0
      };

      try {
        // Execute the action
        await step.action(page);

        // Validate if validation function provided
        if (step.validate) {
          result.passed = await step.validate(page);
        } else {
          result.passed = true;
        }

        // Capture screenshot if requested or if step failed
        if (step.captureScreenshot || !result.passed) {
          result.screenshot = await this.captureScreenshot(page, step.name);
        }

        if (!result.passed) {
          allPassed = false;
        }
      } catch (error) {
        result.passed = false;
        result.error = error instanceof Error ? error.message : String(error);
        allPassed = false;

        // Always capture screenshot on error
        result.screenshot = await this.captureScreenshot(page, `${step.name}-ERROR`);
      }

      result.duration = Date.now() - stepStartTime;
      results.push(result);

      // Stop on first failure unless configured otherwise
      if (!result.passed) {
        console.error(`❌ Step failed: ${step.name}`);
        if (result.error) {
          console.error(`   Error: ${result.error}`);
        }
      } else {
        console.log(`✅ Step passed: ${step.name}`);
      }
    }

    const endTime = new Date();

    return {
      testName: this.testName,
      passed: allPassed,
      steps: results,
      startTime,
      endTime,
      totalDuration: endTime.getTime() - startTime.getTime(),
      screenshotDir: this.screenshotDir
    };
  }

  async navigateAndWait(page: Page, route: string): Promise<void> {
    await page.goto(`${this.baseUrl}${route}`, {
      waitUntil: 'networkidle'
    });
  }

  async waitForElement(page: Page, selector: string, timeout: number = 5000): Promise<void> {
    await page.waitForSelector(selector, { timeout });
  }

  async clickAndWait(page: Page, selector: string): Promise<void> {
    await page.click(selector);
    await page.waitForLoadState('networkidle');
  }

  async typeAndWait(page: Page, selector: string, text: string): Promise<void> {
    await page.fill(selector, text);
    await page.waitForTimeout(500); // Wait for any immediate reactions
  }

  generateHTMLReport(report: TestReport): string {
    const reportPath = path.join(
      process.cwd(),
      'test-orchestrator',
      'reports',
      `${this.testName}-${new Date().toISOString().replace(/[:.]/g, '-')}.html`
    );

    const html = `
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Test Report: ${report.testName}</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }
        .header {
            background: ${report.passed ? '#4caf50' : '#f44336'};
            color: white;
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 20px;
        }
        .summary {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 10px;
            margin: 20px 0;
        }
        .summary-item {
            background: white;
            padding: 15px;
            border-radius: 6px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .summary-label {
            font-size: 12px;
            color: #666;
            margin-bottom: 5px;
        }
        .summary-value {
            font-size: 24px;
            font-weight: bold;
        }
        .step {
            background: white;
            margin: 10px 0;
            padding: 15px;
            border-radius: 6px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .step.passed {
            border-left: 4px solid #4caf50;
        }
        .step.failed {
            border-left: 4px solid #f44336;
        }
        .step-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 10px;
        }
        .step-name {
            font-weight: bold;
            font-size: 16px;
        }
        .step-status {
            padding: 4px 12px;
            border-radius: 4px;
            font-size: 12px;
            font-weight: bold;
        }
        .step-status.passed {
            background: #e8f5e9;
            color: #2e7d32;
        }
        .step-status.failed {
            background: #ffebee;
            color: #c62828;
        }
        .step-error {
            background: #ffebee;
            color: #c62828;
            padding: 10px;
            border-radius: 4px;
            margin: 10px 0;
            font-family: monospace;
            font-size: 12px;
        }
        .screenshot {
            margin: 10px 0;
        }
        .screenshot img {
            max-width: 100%;
            border: 1px solid #ddd;
            border-radius: 4px;
            cursor: pointer;
        }
        .screenshot img:hover {
            opacity: 0.9;
        }
        .step-meta {
            font-size: 12px;
            color: #666;
            margin-top: 10px;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>${report.passed ? '✅' : '❌'} ${report.testName}</h1>
        <p>Test ${report.passed ? 'PASSED' : 'FAILED'}</p>
    </div>

    <div class="summary">
        <div class="summary-item">
            <div class="summary-label">Total Steps</div>
            <div class="summary-value">${report.steps.length}</div>
        </div>
        <div class="summary-item">
            <div class="summary-label">Passed</div>
            <div class="summary-value" style="color: #4caf50">${report.steps.filter(s => s.passed).length}</div>
        </div>
        <div class="summary-item">
            <div class="summary-label">Failed</div>
            <div class="summary-value" style="color: #f44336">${report.steps.filter(s => !s.passed).length}</div>
        </div>
        <div class="summary-item">
            <div class="summary-label">Duration</div>
            <div class="summary-value">${(report.totalDuration / 1000).toFixed(2)}s</div>
        </div>
    </div>

    <h2>Test Steps</h2>
    ${report.steps.map((step, index) => `
        <div class="step ${step.passed ? 'passed' : 'failed'}">
            <div class="step-header">
                <div class="step-name">${index + 1}. ${step.stepName}</div>
                <div class="step-status ${step.passed ? 'passed' : 'failed'}">${step.passed ? 'PASSED' : 'FAILED'}</div>
            </div>
            ${step.error ? `<div class="step-error">${step.error}</div>` : ''}
            ${step.screenshot ? `
                <div class="screenshot">
                    <img src="${path.relative(path.dirname(reportPath), step.screenshot)}"
                         alt="Screenshot for ${step.stepName}"
                         onclick="window.open(this.src)">
                </div>
            ` : ''}
            <div class="step-meta">
                Duration: ${step.duration}ms |
                Timestamp: ${step.timestamp.toISOString()}
            </div>
        </div>
    `).join('')}

    <div style="margin-top: 40px; padding: 20px; background: white; border-radius: 6px;">
        <h3>Test Information</h3>
        <p><strong>Start Time:</strong> ${report.startTime.toISOString()}</p>
        <p><strong>End Time:</strong> ${report.endTime.toISOString()}</p>
        <p><strong>Screenshots Directory:</strong> ${report.screenshotDir}</p>
    </div>
</body>
</html>
    `.trim();

    fs.writeFileSync(reportPath, html);
    console.log(`\n📄 HTML Report generated: ${reportPath}`);

    return reportPath;
  }
}
