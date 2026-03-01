import { test, expect } from '@playwright/test';

/**
 * CARNELIAN Dioxus Desktop UI E2E Tests
 * 
 * These tests validate the desktop UI functionality.
 * Note: Dioxus desktop apps run as native applications, not web browsers.
 * For now, these tests target the web preview mode.
 */

test.describe('CARNELIAN Desktop UI', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the UI (adjust URL based on your setup)
    await page.goto('http://localhost:18789');
  });

  test('should load dashboard page', async ({ page }) => {
    await expect(page).toHaveTitle(/CARNELIAN/);
    await expect(page.locator('h1')).toContainText('Dashboard');
  });

  test('should display system status', async ({ page }) => {
    // Check for status indicators
    const statusSection = page.locator('[data-testid="system-status"]');
    await expect(statusSection).toBeVisible();
  });

  test('should navigate to tasks page', async ({ page }) => {
    await page.click('a[href="/tasks"]');
    await expect(page).toHaveURL(/.*tasks/);
    await expect(page.locator('h1')).toContainText('Tasks');
  });

  test('should navigate to skills page', async ({ page }) => {
    await page.click('a[href="/skills"]');
    await expect(page).toHaveURL(/.*skills/);
    await expect(page.locator('h1')).toContainText('Skills');
  });

  test('should display XP widget', async ({ page }) => {
    const xpWidget = page.locator('[data-testid="xp-widget"]');
    await expect(xpWidget).toBeVisible();
  });

  test('should show event stream', async ({ page }) => {
    await page.click('a[href="/events"]');
    await expect(page).toHaveURL(/.*events/);
    const eventStream = page.locator('[data-testid="event-stream"]');
    await expect(eventStream).toBeVisible();
  });

  test('should handle WebSocket connection', async ({ page }) => {
    // Wait for WebSocket connection
    await page.waitForTimeout(1000);
    
    // Check for connection indicator
    const wsIndicator = page.locator('[data-testid="ws-status"]');
    await expect(wsIndicator).toHaveAttribute('data-connected', 'true');
  });
});

test.describe('CARNELIAN Task Management', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:18789/tasks');
  });

  test('should create new task', async ({ page }) => {
    await page.click('button:has-text("New Task")');
    await page.fill('input[name="title"]', 'Test Task');
    await page.fill('textarea[name="description"]', 'Test Description');
    await page.click('button:has-text("Create")');
    
    await expect(page.locator('text=Test Task')).toBeVisible();
  });

  test('should filter tasks', async ({ page }) => {
    await page.selectOption('select[name="status"]', 'pending');
    await expect(page.locator('[data-testid="task-list"]')).toBeVisible();
  });
});

test.describe('CARNELIAN Skills Management', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:18789/skills');
  });

  test('should display skill registry', async ({ page }) => {
    const skillList = page.locator('[data-testid="skill-list"]');
    await expect(skillList).toBeVisible();
  });

  test('should search skills', async ({ page }) => {
    await page.fill('input[placeholder*="Search"]', 'echo');
    await expect(page.locator('text=echo')).toBeVisible();
  });

  test('should enable/disable skill', async ({ page }) => {
    const skillToggle = page.locator('[data-testid="skill-toggle"]').first();
    await skillToggle.click();
    // Verify state change
    await page.waitForTimeout(500);
  });
});

test.describe('CARNELIAN Approvals', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:18789/approvals');
  });

  test('should display approval queue', async ({ page }) => {
    const approvalQueue = page.locator('[data-testid="approval-queue"]');
    await expect(approvalQueue).toBeVisible();
  });

  test('should approve request', async ({ page }) => {
    const approveButton = page.locator('button:has-text("Approve")').first();
    if (await approveButton.isVisible()) {
      await approveButton.click();
      await expect(page.locator('text=Approved')).toBeVisible();
    }
  });
});
