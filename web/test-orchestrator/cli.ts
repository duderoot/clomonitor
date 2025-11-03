#!/usr/bin/env node

/**
 * TDD Orchestrator CLI
 *
 * This CLI tool coordinates TDD workflows between UI development and Playwright testing.
 *
 * Usage:
 *   npm run tdd-test <workflow-name> [options]
 *
 * Examples:
 *   npm run tdd-test dt-integration
 *   npm run tdd-test project-search --cycles 3
 *   npm run tdd-test dt-integration --headless false
 */

import { TDDCoordinator } from './helpers/tdd-coordinator.ts';
import { dtIntegrationWorkflow, projectSearchWorkflow } from './workflows/example-dt-integration.ts';
import { homepageNavigationWorkflow } from './workflows/homepage-navigation.ts';
import { dtVisibilityPageWorkflow } from './workflows/dt-visibility-page.ts';
import { dtErrorDiagnosticWorkflow } from './workflows/dt-error-diagnostic.ts';
import { dtOverviewValidationWorkflow } from './workflows/dt-overview-validation.ts';
import { dtCompleteFlowWorkflow } from './workflows/dt-complete-flow.ts';

const WORKFLOWS = {
  'dt-integration': dtIntegrationWorkflow,
  'dt-complete': dtCompleteFlowWorkflow,
  'project-search': projectSearchWorkflow,
  'homepage': homepageNavigationWorkflow,
  'dt-visibility': dtVisibilityPageWorkflow,
  'dt-error': dtErrorDiagnosticWorkflow,
  'dt-overview': dtOverviewValidationWorkflow
};

interface CLIOptions {
  workflow?: string;
  cycles?: number;
  baseUrl?: string;
  headless?: boolean;
  help?: boolean;
}

function parseArgs(): CLIOptions {
  const args = process.argv.slice(2);
  const options: CLIOptions = {
    cycles: 5,
    baseUrl: 'http://localhost:3000',
    headless: false
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    if (arg === '--help' || arg === '-h') {
      options.help = true;
    } else if (arg === '--cycles') {
      options.cycles = parseInt(args[++i], 10);
    } else if (arg === '--base-url') {
      options.baseUrl = args[++i];
    } else if (arg === '--headless') {
      options.headless = args[++i].toLowerCase() === 'true';
    } else if (!arg.startsWith('--')) {
      options.workflow = arg;
    }
  }

  return options;
}

function printHelp(): void {
  console.log(`
╔═══════════════════════════════════════════════════════════════════╗
║                    TDD Orchestrator CLI                           ║
╚═══════════════════════════════════════════════════════════════════╝

USAGE:
  npm run tdd-test <workflow-name> [options]

WORKFLOWS:
  dt-complete        🎯 Phase 1 acceptance test (end-to-end DT visibility flow)
  dt-integration     Test DT visibility toggle feature
  dt-overview        Validate Overview dashboard (expects dt_import_history data)
  dt-error           Diagnose errors on DT visibility page
  dt-visibility      Test DT visibility page navigation
  project-search     Test project search and filtering
  homepage           Test homepage navigation

OPTIONS:
  --cycles <n>       Maximum number of test cycles (default: 5)
  --base-url <url>   Base URL for the application (default: http://localhost:3000)
  --headless <bool>  Run browser in headless mode (default: false)
  --help, -h         Show this help message

EXAMPLES:
  # Run DT integration workflow
  npm run tdd-test dt-integration

  # Run with custom cycles
  npm run tdd-test project-search --cycles 3

  # Run in headless mode
  npm run tdd-test dt-integration --headless true

WORKFLOW:
  1. The orchestrator runs the test workflow
  2. Tests fail (Red phase)
  3. Review screenshots and error messages
  4. Make UI changes to implement the feature
  5. Run tests again
  6. Repeat until tests pass (Green phase)
  7. Refactor if needed

REPORTS:
  - HTML reports: test-orchestrator/reports/*.html
  - Screenshots: test-orchestrator/reports/screenshots/*
  - Videos: test-orchestrator/reports/videos/*
  - JSON summary: test-orchestrator/reports/tdd-session-*.json
`);
}

function listAvailableWorkflows(): void {
  console.log('\nAvailable workflows:\n');
  Object.entries(WORKFLOWS).forEach(([name, workflow]) => {
    console.log(`  📋 ${name}`);
    console.log(`     ${workflow.description}`);
    console.log(`     Feature: ${workflow.feature}\n`);
  });
}

async function main(): Promise<void> {
  const options = parseArgs();

  if (options.help) {
    printHelp();
    process.exit(0);
  }

  if (!options.workflow) {
    console.error('❌ Error: No workflow specified\n');
    listAvailableWorkflows();
    console.log('Use --help for more information\n');
    process.exit(1);
  }

  const workflow = WORKFLOWS[options.workflow as keyof typeof WORKFLOWS];

  if (!workflow) {
    console.error(`❌ Error: Unknown workflow "${options.workflow}"\n`);
    listAvailableWorkflows();
    process.exit(1);
  }

  console.log(`
╔═══════════════════════════════════════════════════════════════════╗
║              Starting TDD Orchestrator                            ║
╚═══════════════════════════════════════════════════════════════════╝

Workflow: ${workflow.name}
Description: ${workflow.description}
Base URL: ${options.baseUrl}
Max Cycles: ${options.cycles}
Headless: ${options.headless}
  `);

  const coordinator = new TDDCoordinator(options.baseUrl, options.headless);

  try {
    await coordinator.initialize();

    const results = await coordinator.runMultipleCycles(workflow, options.cycles);

    const sessionReportPath = coordinator.generateFullReport();

    console.log(`
╔═══════════════════════════════════════════════════════════════════╗
║                    TDD Session Complete                           ║
╚═══════════════════════════════════════════════════════════════════╝

Total Cycles: ${results.length}
Final Status: ${results[results.length - 1].passed ? '✅ PASSED' : '❌ FAILED'}
Session Report: ${sessionReportPath}

Review the HTML reports and screenshots for detailed information.
    `);

    process.exit(results[results.length - 1].passed ? 0 : 1);
  } catch (error) {
    console.error('\n❌ Fatal error:', error);
    process.exit(1);
  } finally {
    await coordinator.cleanup();
  }
}

// Handle graceful shutdown
process.on('SIGINT', async () => {
  console.log('\n\n🛑 Received SIGINT, shutting down gracefully...');
  process.exit(130);
});

process.on('SIGTERM', async () => {
  console.log('\n\n🛑 Received SIGTERM, shutting down gracefully...');
  process.exit(143);
});

main().catch((error) => {
  console.error('Unhandled error:', error);
  process.exit(1);
});
