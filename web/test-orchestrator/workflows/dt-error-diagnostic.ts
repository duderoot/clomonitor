import { TDDWorkflow } from '../helpers/tdd-coordinator.ts';
import { Page } from '@playwright/test';

/**
 * Quick diagnostic workflow to capture the error on DT visibility page
 */
export const dtErrorDiagnosticWorkflow: TDDWorkflow = {
  name: 'dt-error-diagnostic',
  description: 'Diagnose error on DT visibility page with unmapped tab',
  feature: 'Capture current error state and identify fixes needed',

  testSteps: [
    {
      name: 'Navigate to DT Visibility page with unmapped tab',
      action: async (page: Page) => {
        await page.goto('http://localhost:3000/dt-visibility?tab=unmapped', {
          waitUntil: 'networkidle',
          timeout: 30000
        });
      },
      validate: async (page: Page) => {
        const url = page.url();
        return url.includes('/dt-visibility');
      },
      captureScreenshot: true
    },

    {
      name: 'Check for React error boundary',
      action: async (page: Page) => {
        await page.waitForTimeout(2000);
      },
      validate: async (page: Page) => {
        const errorText = await page.textContent('body');
        const hasError = errorText?.includes('error') || errorText?.includes('Error');
        // We want this to fail if there's an error, so we return the opposite
        return !hasError;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify page renders without errors',
      action: async (page: Page) => {
        await page.waitForTimeout(1000);
      },
      validate: async (page: Page) => {
        // Check that the main content is visible
        const heading = await page.textContent('.h2, h2, h1');
        return heading !== null && heading.length > 0;
      },
      captureScreenshot: true
    }
  ],

  uiChangesRequired: [
    'Fix any runtime errors in DT components',
    'Ensure API endpoints return valid data',
    'Add proper error boundaries',
    'Handle loading and error states'
  ]
};
