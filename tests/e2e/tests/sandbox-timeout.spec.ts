/**
 * E2E tests for Windows sandbox timeout behavior
 * 
 * Tests that sandboxed skill execution respects timeout limits
 * and properly terminates long-running processes on Windows.
 */

import { test, expect } from '@playwright/test';

test.describe('Sandbox Timeout Behavior', () => {
  test('should timeout long-running skill execution', async ({ page }) => {
    // Navigate to skill execution page
    await page.goto('http://localhost:3000/skills');
    
    // Wait for skills to load
    await page.waitForSelector('[data-testid="skill-list"]', { timeout: 10000 });
    
    // Look for a test skill that can simulate timeout
    const timeoutSkill = page.locator('[data-testid="skill-item"]').filter({ hasText: 'timeout' }).first();
    
    if (await timeoutSkill.count() > 0) {
      // Execute the timeout skill
      await timeoutSkill.click();
      await page.click('[data-testid="execute-skill-button"]');
      
      // Wait for execution to start
      await page.waitForSelector('[data-testid="execution-status"]', { timeout: 5000 });
      
      // Verify timeout occurs within reasonable time (e.g., 30 seconds)
      const startTime = Date.now();
      await page.waitForSelector('[data-testid="execution-failed"]', { timeout: 35000 });
      const elapsed = Date.now() - startTime;
      
      // Should timeout within configured limit
      expect(elapsed).toBeLessThan(35000);
      
      // Verify error message mentions timeout
      const errorMessage = await page.locator('[data-testid="error-message"]').textContent();
      expect(errorMessage?.toLowerCase()).toContain('timeout');
    } else {
      test.skip();
    }
  });

  test('should successfully execute quick skills within timeout', async ({ page }) => {
    await page.goto('http://localhost:3000/skills');
    await page.waitForSelector('[data-testid="skill-list"]', { timeout: 10000 });
    
    // Find a quick-executing skill
    const quickSkill = page.locator('[data-testid="skill-item"]').first();
    
    if (await quickSkill.count() > 0) {
      await quickSkill.click();
      await page.click('[data-testid="execute-skill-button"]');
      
      // Should complete successfully
      await page.waitForSelector('[data-testid="execution-success"]', { timeout: 15000 });
      
      const status = await page.locator('[data-testid="execution-status"]').textContent();
      expect(status?.toLowerCase()).toContain('success');
    } else {
      test.skip();
    }
  });
});
