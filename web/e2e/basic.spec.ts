import { test, expect } from '@playwright/test';

test.describe('ferncad web app', () => {
  test('loads and shows ready status', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-bar')).toContainText('triangles', {
      timeout: 10000,
    });
  });

  test('displays toolbar buttons', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#btn-evaluate')).toBeVisible();
    await expect(page.locator('#btn-export-stl')).toBeVisible();
  });

  test('shows error for invalid syntax', async ({ page }) => {
    await page.goto('/');
    // Wait for WASM to initialize
    await expect(page.locator('#status-bar')).not.toContainText('Initializing', {
      timeout: 10000,
    });

    // Clear editor and type invalid code
    const editor = page.locator('.cm-content');
    await editor.click();
    await page.keyboard.press('Meta+a');
    await page.keyboard.type('(invalid syntax');

    // Trigger evaluation
    await page.click('#btn-evaluate');

    // Should show error
    await expect(page.locator('#status-bar')).toHaveClass(/error/, {
      timeout: 5000,
    });
  });

  test('evaluates valid code and renders mesh', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-bar')).not.toContainText('Initializing', {
      timeout: 10000,
    });

    const editor = page.locator('.cm-content');
    await editor.click();
    await page.keyboard.press('Meta+a');
    await page.keyboard.type('(sphere :radius 5)');

    await page.click('#btn-evaluate');

    await expect(page.locator('#status-bar')).toContainText('triangles', {
      timeout: 5000,
    });
    await expect(page.locator('#status-bar')).toHaveClass(/success/);
  });

  test('canvas element exists', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#viewer-canvas')).toBeVisible();
  });
});
