import { TDDWorkflow } from '../helpers/tdd-coordinator.ts';
import { Page } from '@playwright/test';

/**
 * DT Visibility Page Workflow - Tests the complete DT unmapped components page
 * This validates the existing implementation that shows libraries that couldn't be imported from Dependency Track
 */
export const dtVisibilityPageWorkflow: TDDWorkflow = {
  name: 'dt-visibility-page',
  description: 'Test DT Visibility page - unmapped components from Dependency Track',
  feature: 'Verify DT Visibility page loads and displays unmapped components correctly',

  testSteps: [
    {
      name: 'Navigate to DT Visibility page',
      action: async (page: Page) => {
        await page.goto('http://localhost:3000/dt-visibility', {
          waitUntil: 'networkidle'
        });
      },
      validate: async (page: Page) => {
        const url = page.url();
        return url.includes('/dt-visibility');
      },
      captureScreenshot: true
    },

    {
      name: 'Verify page title and description',
      action: async (page: Page) => {
        await page.waitForTimeout(1000);
      },
      validate: async (page: Page) => {
        const title = await page.textContent('.h2');
        const hasTitle = title?.includes('Dependency-Track Import Visibility') || false;

        const description = await page.textContent('.text-muted');
        const hasDescription = description?.includes('Monitor component import') || false;

        return hasTitle && hasDescription;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify foundation selector exists',
      action: async (page: Page) => {
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const selector = await page.$('select.foundation');
        return selector !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify tabs exist (Overview and Unmapped)',
      action: async (page: Page) => {
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const overviewTab = await page.textContent('.nav-tabs .nav-link:has-text("Overview")');
        const unmappedTab = await page.textContent('.nav-tabs .nav-link:has-text("Unmapped")');

        return overviewTab !== null && unmappedTab !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Click on Unmapped Components tab',
      action: async (page: Page) => {
        await page.click('.nav-tabs .nav-link:has-text("Unmapped")');
        await page.waitForTimeout(1000);
      },
      validate: async (page: Page) => {
        const activeTab = await page.$('.nav-tabs .nav-link.active:has-text("Unmapped")');
        return activeTab !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify Unmapped Components section header',
      action: async (page: Page) => {
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const heading = await page.textContent('h5');
        return heading?.includes('Unmapped Components') || false;
      },
      captureScreenshot: true
    },

    {
      name: 'Verify search functionality exists',
      action: async (page: Page) => {
        await page.waitForTimeout(500);
      },
      validate: async (page: Page) => {
        const searchInput = await page.$('input[placeholder*="Search"]');
        const searchButton = await page.$('button[type="submit"]:has-text("Search")');

        return searchInput !== null && searchButton !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Check for table or no data message',
      action: async (page: Page) => {
        await page.waitForTimeout(1000);
      },
      validate: async (page: Page) => {
        // Either table exists or "No unmapped components" message
        const table = await page.$('table');
        const noData = await page.textContent('text="No unmapped components found"');

        return table !== null || noData !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Test foundation filter dropdown',
      action: async (page: Page) => {
        const selector = await page.$('select.foundation');
        if (selector) {
          // Get current value
          const currentValue = await page.$eval('select.foundation', (el: HTMLSelectElement) => el.value);

          // Get all options
          const options = await page.$$eval('select.foundation option',
            (elements: HTMLOptionElement[]) => elements.map(el => el.value).filter(v => v !== '')
          );

          // If there are options other than current, select one
          if (options.length > 0) {
            const newValue = options[0] !== currentValue ? options[0] : (options[1] || options[0]);
            await page.selectOption('select.foundation', newValue);
            await page.waitForTimeout(1500); // Wait for API call
          }
        }
      },
      validate: async (page: Page) => {
        // Just verify the selector still exists and is functional
        const selector = await page.$('select.foundation');
        return selector !== null;
      },
      captureScreenshot: true
    },

    {
      name: 'Test search input functionality',
      action: async (page: Page) => {
        const searchInput = await page.$('input[placeholder*="Search"]');
        if (searchInput) {
          await page.fill('input[placeholder*="Search"]', 'test');
          await page.waitForTimeout(500);
        }
      },
      validate: async (page: Page) => {
        const searchInput = await page.$('input[placeholder*="Search"]');
        if (!searchInput) return false;

        const value = await searchInput.inputValue();
        return value === 'test';
      },
      captureScreenshot: true
    },

    {
      name: 'Clear search input',
      action: async (page: Page) => {
        const clearButton = await page.$('button:has-text("Clear")');
        if (clearButton) {
          await page.click('button:has-text("Clear")');
          await page.waitForTimeout(500);
        } else {
          // If no clear button (because no search was performed), just clear the input
          await page.fill('input[placeholder*="Search"]', '');
        }
      },
      validate: async (page: Page) => {
        const searchInput = await page.$('input[placeholder*="Search"]');
        if (!searchInput) return false;

        const value = await searchInput.inputValue();
        return value === '';
      },
      captureScreenshot: true
    },

    {
      name: 'Navigate back to Overview tab',
      action: async (page: Page) => {
        await page.click('.nav-tabs .nav-link:has-text("Overview")');
        await page.waitForTimeout(1000);
      },
      validate: async (page: Page) => {
        const activeTab = await page.$('.nav-tabs .nav-link.active:has-text("Overview")');
        return activeTab !== null;
      },
      captureScreenshot: true
    }
  ],

  uiChangesRequired: [
    'This workflow tests existing implementation',
    'No changes needed - validating current DT Visibility page functionality'
  ]
};
