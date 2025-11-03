import { TDDWorkflow } from '../helpers/tdd-coordinator.ts';
import { Page } from '@playwright/test';

/**
 * Homepage navigation workflow - tests existing functionality
 * This demonstrates TDD with real pages that exist
 */
export const homepageNavigationWorkflow: TDDWorkflow = {
  name: 'homepage-navigation',
  description: 'Test homepage navigation and search functionality',
  feature: 'Verify homepage loads and basic navigation works',

  testSteps: [
    {
      name: 'Navigate to homepage',
      action: async (page: Page) => {
        await page.goto('http://localhost:3000/', {
          waitUntil: 'networkidle'
        });
      },
      validate: async (page: Page) => {
        const title = await page.textContent('h1, .title, [class*="title"]');
        return title !== null && title.length > 0;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify search box exists',
      action: async (page: Page) => {
        await page.waitForTimeout(1000);
      },
      validate: async (page: Page) => {
        const searchBox = await page.$('input[type="search"], input[placeholder*="search" i], input[name*="search" i]');
        return searchBox !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Check navigation menu',
      action: async (page: Page) => {
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const nav = await page.$('nav, [role="navigation"]');
        return nav !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify footer exists',
      action: async (page: Page) => {
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const footer = await page.$('footer, [role="contentinfo"]');
        return footer !== null;
      },
      captureScreenshot: true
    }
  ],

  uiChangesRequired: [
    'This workflow tests existing functionality',
    'No UI changes needed - validation of current state'
  ]
};
