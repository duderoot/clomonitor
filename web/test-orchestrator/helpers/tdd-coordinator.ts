import { Browser, Page, chromium } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';
import { PlaywrightTestRunner, TestStep, TestReport } from './test-runner.ts';

export interface TDDWorkflow {
  name: string;
  description: string;
  feature: string;
  testSteps: TestStep[];
  uiChangesRequired?: string[];
}

export interface TDDCycleResult {
  workflow: string;
  cycle: number;
  testReport: TestReport;
  passed: boolean;
  nextActions?: string[];
}

export class TDDCoordinator {
  private browser: Browser | null = null;
  private context: any = null;
  private page: Page | null = null;
  private cycleHistory: TDDCycleResult[] = [];

  constructor(
    private baseUrl: string = 'http://localhost:3000',
    private headless: boolean = false
  ) {}

  async initialize(): Promise<void> {
    this.browser = await chromium.launch({
      headless: this.headless,
      slowMo: 100 // Slow down operations for better visibility
    });

    this.context = await this.browser.newContext({
      viewport: { width: 1920, height: 1080 },
      recordVideo: {
        dir: path.join(process.cwd(), 'test-orchestrator', 'reports', 'videos'),
        size: { width: 1920, height: 1080 }
      }
    });

    this.page = await this.context.newPage();

    // Setup console logging
    this.page.on('console', (msg) => {
      console.log(`[Browser Console] ${msg.type()}: ${msg.text()}`);
    });

    // Setup error logging
    this.page.on('pageerror', (error) => {
      console.error(`[Browser Error] ${error.message}`);
    });
  }

  async cleanup(): Promise<void> {
    if (this.page) await this.page.close();
    if (this.context) await this.context.close();
    if (this.browser) await this.browser.close();
  }

  async runTDDCycle(workflow: TDDWorkflow): Promise<TDDCycleResult> {
    if (!this.page) {
      throw new Error('TDDCoordinator not initialized. Call initialize() first.');
    }

    const cycleNumber = this.cycleHistory.filter(h => h.workflow === workflow.name).length + 1;

    console.log(`\n${'='.repeat(80)}`);
    console.log(`🔄 TDD Cycle ${cycleNumber}: ${workflow.name}`);
    console.log(`📝 Feature: ${workflow.feature}`);
    console.log(`${'='.repeat(80)}\n`);

    const testRunner = new PlaywrightTestRunner(
      `${workflow.name}-cycle${cycleNumber}`,
      this.baseUrl
    );

    const testReport = await testRunner.runSteps(this.page, workflow.testSteps);

    const htmlReportPath = testRunner.generateHTMLReport(testReport);

    const result: TDDCycleResult = {
      workflow: workflow.name,
      cycle: cycleNumber,
      testReport,
      passed: testReport.passed,
      nextActions: testReport.passed ? undefined : this.generateNextActions(testReport, workflow)
    };

    this.cycleHistory.push(result);

    this.printCycleSummary(result);

    return result;
  }

  private generateNextActions(report: TestReport, workflow: TDDWorkflow): string[] {
    const failedSteps = report.steps.filter(s => !s.passed);
    const actions: string[] = [];

    actions.push('🔧 UI Changes Required:');

    for (const step of failedSteps) {
      if (step.error) {
        actions.push(`  - Fix error in "${step.stepName}": ${step.error}`);
      } else {
        actions.push(`  - Implement validation for "${step.stepName}"`);
      }
    }

    if (workflow.uiChangesRequired) {
      actions.push('\n📋 Suggested UI Changes:');
      workflow.uiChangesRequired.forEach(change => {
        actions.push(`  - ${change}`);
      });
    }

    actions.push('\n🔄 Next Steps:');
    actions.push('  1. Review screenshots in: ' + report.screenshotDir);
    actions.push('  2. Make necessary UI changes');
    actions.push('  3. Run the test cycle again');

    return actions;
  }

  private printCycleSummary(result: TDDCycleResult): void {
    console.log(`\n${'='.repeat(80)}`);
    console.log(`📊 Cycle ${result.cycle} Summary for "${result.workflow}"`);
    console.log(`${'='.repeat(80)}`);
    console.log(`Status: ${result.passed ? '✅ PASSED' : '❌ FAILED'}`);
    console.log(`Total Steps: ${result.testReport.steps.length}`);
    console.log(`Passed: ${result.testReport.steps.filter(s => s.passed).length}`);
    console.log(`Failed: ${result.testReport.steps.filter(s => !s.passed).length}`);
    console.log(`Duration: ${(result.testReport.totalDuration / 1000).toFixed(2)}s`);

    if (result.nextActions) {
      console.log(`\n${result.nextActions.join('\n')}`);
    }

    console.log(`\n📸 Screenshots: ${result.testReport.screenshotDir}`);
    console.log(`${'='.repeat(80)}\n`);
  }

  async runMultipleCycles(
    workflow: TDDWorkflow,
    maxCycles: number = 5
  ): Promise<TDDCycleResult[]> {
    const results: TDDCycleResult[] = [];

    for (let i = 0; i < maxCycles; i++) {
      const result = await this.runTDDCycle(workflow);
      results.push(result);

      if (result.passed) {
        console.log(`\n🎉 Workflow "${workflow.name}" completed successfully in ${i + 1} cycle(s)!`);
        break;
      }

      if (i < maxCycles - 1) {
        console.log(`\n⏸️  Pausing for UI changes. Press any key to continue to cycle ${i + 2}...`);
        // In real usage, this would wait for user input
        await this.waitForUserInput();
      }
    }

    return results;
  }

  private async waitForUserInput(): Promise<void> {
    // This is a placeholder - in real implementation, this would wait for user input
    // For automated testing, we just continue
    return Promise.resolve();
  }

  getCycleHistory(): TDDCycleResult[] {
    return this.cycleHistory;
  }

  generateFullReport(): string {
    const reportPath = path.join(
      process.cwd(),
      'test-orchestrator',
      'reports',
      `tdd-session-${new Date().toISOString().replace(/[:.]/g, '-')}.json`
    );

    const report = {
      sessionStart: this.cycleHistory[0]?.testReport.startTime,
      sessionEnd: this.cycleHistory[this.cycleHistory.length - 1]?.testReport.endTime,
      totalCycles: this.cycleHistory.length,
      workflows: [...new Set(this.cycleHistory.map(c => c.workflow))],
      cycles: this.cycleHistory.map(c => ({
        workflow: c.workflow,
        cycle: c.cycle,
        passed: c.passed,
        duration: c.testReport.totalDuration,
        failedSteps: c.testReport.steps.filter(s => !s.passed).map(s => s.stepName),
        screenshotDir: c.testReport.screenshotDir
      }))
    };

    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
    console.log(`\n📄 Full TDD session report saved: ${reportPath}`);

    return reportPath;
  }
}
