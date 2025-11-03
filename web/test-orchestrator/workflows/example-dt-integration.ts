import { TDDWorkflow } from '../helpers/tdd-coordinator.ts';
import { Page } from '@playwright/test';

/**
 * Example TDD workflow for DT (Dependency Track) integration feature
 * This demonstrates how to create a test-first workflow
 */
export const dtIntegrationWorkflow: TDDWorkflow = {
  name: 'dt-integration-visibility',
  description: 'Test DT visibility toggle in project detail page',
  feature: 'Add DT visibility toggle to enable/disable DT data in project details',

  testSteps: [
    {
      name: 'Navigate to project detail page',
      action: async (page: Page) => {
        await page.goto('http://localhost:3000/projects/cncf/kubernetes', {
          waitUntil: 'networkidle'
        });
      },
      validate: async (page: Page) => {
        return await page.isVisible('h1');
      },
      captureScreenshot: true
    },

    {
      name: 'Verify DT toggle exists',
      action: async (page: Page) => {
        // Wait for the DT toggle to be visible
        await page.waitForSelector('[data-testid="dt-visibility-toggle"]', {
          timeout: 5000
        });
      },
      validate: async (page: Page) => {
        const toggle = await page.$('[data-testid="dt-visibility-toggle"]');
        return toggle !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Toggle DT visibility ON',
      action: async (page: Page) => {
        await page.click('[data-testid="dt-visibility-toggle"]');
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const checkbox = await page.$('[data-testid="dt-visibility-toggle"] input[type="checkbox"]');
        return await checkbox?.isChecked() ?? false;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify DT data is displayed',
      action: async (page: Page) => {
        await page.waitForSelector('[data-testid="dt-data-section"]', {
          timeout: 5000
        });
      },
      validate: async (page: Page) => {
        const dtSection = await page.$('[data-testid="dt-data-section"]');
        return dtSection !== null && await dtSection.isVisible();
      },
      captureScreenshot: true
    },

    {
      name: 'Toggle DT visibility OFF',
      action: async (page: Page) => {
        await page.click('[data-testid="dt-visibility-toggle"]');
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const checkbox = await page.$('[data-testid="dt-visibility-toggle"] input[type="checkbox"]');
        const isChecked = await checkbox?.isChecked() ?? true;
        return !isChecked;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify DT data is hidden',
      action: async (page: Page) => {
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const dtSection = await page.$('[data-testid="dt-data-section"]');
        if (!dtSection) return true;
        return !(await dtSection.isVisible());
      },
      captureScreenshot: true
    }
  ],

  uiChangesRequired: [
    'Add DT visibility toggle component in project detail page',
    'Implement toggle state management using React context or local state',
    'Conditionally render DT data section based on toggle state',
    'Add data-testid attributes to components for testing',
    'Persist toggle state in localStorage or user preferences'
  ]
};

/**
 * Example workflow for testing project search functionality
 */
export const projectSearchWorkflow: TDDWorkflow = {
  name: 'project-search',
  description: 'Test project search and filtering',
  feature: 'Search and filter projects in the projects list',

  testSteps: [
    {
      name: 'Navigate to projects page',
      action: async (page: Page) => {
        await page.goto('http://localhost:3000/projects', {
          waitUntil: 'networkidle'
        });
      },
      validate: async (page: Page) => {
        return await page.isVisible('[data-testid="projects-list"]');
      },
      captureScreenshot: true
    },

    {
      name: 'Enter search query',
      action: async (page: Page) => {
        await page.fill('[data-testid="search-input"]', 'kubernetes');
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const input = await page.$('[data-testid="search-input"]');
        const value = await input?.inputValue();
        return value === 'kubernetes';
      },
      captureScreenshot: true
    },

    {
      name: 'Verify filtered results',
      action: async (page: Page) => {
        await page.waitForSelector('[data-testid="project-card"]', {
          timeout: 5000
        });
      },
      validate: async (page: Page) => {
        const projectCards = await page.$$('[data-testid="project-card"]');
        // Should have at least one result
        return projectCards.length > 0;
      },
      captureScreenshot: true
    },

    {
      name: 'Clear search',
      action: async (page: Page) => {
        await page.fill('[data-testid="search-input"]', '');
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const projectCards = await page.$$('[data-testid="project-card"]');
        // Should have more results than when filtered
        return projectCards.length > 1;
      },
      captureScreenshot: true
    }
  ],

  uiChangesRequired: [
    'Add search input component with data-testid="search-input"',
    'Implement search filtering logic',
    'Add project card components with data-testid="project-card"',
    'Debounce search input for better performance'
  ]
};
