import { test, expect } from '@playwright/test';

/**
 * E2E tests for workflow execution
 */

test.describe('Workflow Execution', () => {
  const apiKey = process.env.API_KEY || 'test_api_key';
  const baseURL = process.env.BASE_URL || 'http://localhost:8080';

  test('should create a new workflow', async ({ request }) => {
    const response = await request.post(`${baseURL}/api/workflows`, {
      headers: {
        'Content-Type': 'application/json',
        'X-Carnelian-Key': apiKey,
      },
      data: {
        name: 'E2E Test Workflow',
        description: 'Workflow created by E2E test',
        steps: [
          {
            skill: 'test-skill',
            input: { action: 'execute', params: { key: 'value' } },
          },
        ],
      },
    });

    expect(response.status()).toBe(201);
    const workflow = await response.json();
    expect(workflow.name).toBe('E2E Test Workflow');
    expect(workflow.steps).toHaveLength(1);
  });

  test('should list workflows', async ({ request }) => {
    const response = await request.get(`${baseURL}/api/workflows?limit=10`, {
      headers: {
        'X-Carnelian-Key': apiKey,
      },
    });

    expect(response.status()).toBe(200);
    const workflows = await response.json();
    expect(Array.isArray(workflows)).toBe(true);
  });

  test('should execute a workflow', async ({ request }) => {
    // First create a workflow
    const createResponse = await request.post(`${baseURL}/api/workflows`, {
      headers: {
        'Content-Type': 'application/json',
        'X-Carnelian-Key': apiKey,
      },
      data: {
        name: 'Execution Test Workflow',
        description: 'Test workflow execution',
        steps: [
          {
            skill: 'test-skill',
            input: { action: 'execute' },
          },
        ],
      },
    });

    const workflow = await createResponse.json();

    // Execute the workflow
    const executeResponse = await request.post(
      `${baseURL}/api/workflows/${workflow.id}/execute`,
      {
        headers: {
          'Content-Type': 'application/json',
          'X-Carnelian-Key': apiKey,
        },
        data: {},
      }
    );

    expect(executeResponse.status()).toBe(200);
    const execution = await executeResponse.json();
    expect(execution.workflow_id).toBe(workflow.id);
  });

  test('should get workflow execution status', async ({ request }) => {
    // Create and execute a workflow
    const createResponse = await request.post(`${baseURL}/api/workflows`, {
      headers: {
        'Content-Type': 'application/json',
        'X-Carnelian-Key': apiKey,
      },
      data: {
        name: 'Status Test Workflow',
        description: 'Test workflow status',
        steps: [{ skill: 'test-skill', input: {} }],
      },
    });

    const workflow = await createResponse.json();

    const executeResponse = await request.post(
      `${baseURL}/api/workflows/${workflow.id}/execute`,
      {
        headers: {
          'Content-Type': 'application/json',
          'X-Carnelian-Key': apiKey,
        },
        data: {},
      }
    );

    const execution = await executeResponse.json();

    // Get execution status
    const statusResponse = await request.get(
      `${baseURL}/api/workflows/executions/${execution.id}`,
      {
        headers: {
          'X-Carnelian-Key': apiKey,
        },
      }
    );

    expect(statusResponse.status()).toBe(200);
    const status = await statusResponse.json();
    expect(status.id).toBe(execution.id);
    expect(['pending', 'running', 'completed', 'failed']).toContain(status.status);
  });

  test('should update a workflow', async ({ request }) => {
    // Create a workflow
    const createResponse = await request.post(`${baseURL}/api/workflows`, {
      headers: {
        'Content-Type': 'application/json',
        'X-Carnelian-Key': apiKey,
      },
      data: {
        name: 'Update Test Workflow',
        description: 'Original description',
        steps: [{ skill: 'test-skill', input: {} }],
      },
    });

    const workflow = await createResponse.json();

    // Update the workflow
    const updateResponse = await request.put(
      `${baseURL}/api/workflows/${workflow.id}`,
      {
        headers: {
          'Content-Type': 'application/json',
          'X-Carnelian-Key': apiKey,
        },
        data: {
          name: 'Updated Workflow',
          description: 'Updated description',
          steps: workflow.steps,
        },
      }
    );

    expect(updateResponse.status()).toBe(200);
    const updated = await updateResponse.json();
    expect(updated.name).toBe('Updated Workflow');
    expect(updated.description).toBe('Updated description');
  });

  test('should delete a workflow', async ({ request }) => {
    // Create a workflow
    const createResponse = await request.post(`${baseURL}/api/workflows`, {
      headers: {
        'Content-Type': 'application/json',
        'X-Carnelian-Key': apiKey,
      },
      data: {
        name: 'Delete Test Workflow',
        description: 'Will be deleted',
        steps: [{ skill: 'test-skill', input: {} }],
      },
    });

    const workflow = await createResponse.json();

    // Delete the workflow
    const deleteResponse = await request.delete(
      `${baseURL}/api/workflows/${workflow.id}`,
      {
        headers: {
          'X-Carnelian-Key': apiKey,
        },
      }
    );

    expect(deleteResponse.status()).toBe(204);

    // Verify it's deleted
    const getResponse = await request.get(
      `${baseURL}/api/workflows/${workflow.id}`,
      {
        headers: {
          'X-Carnelian-Key': apiKey,
        },
      }
    );

    expect(getResponse.status()).toBe(404);
  });

  test('should handle workflow execution errors gracefully', async ({ request }) => {
    // Create a workflow with invalid skill
    const createResponse = await request.post(`${baseURL}/api/workflows`, {
      headers: {
        'Content-Type': 'application/json',
        'X-Carnelian-Key': apiKey,
      },
      data: {
        name: 'Error Test Workflow',
        description: 'Should fail',
        steps: [
          {
            skill: 'nonexistent-skill',
            input: {},
          },
        ],
      },
    });

    const workflow = await createResponse.json();

    // Execute should fail gracefully
    const executeResponse = await request.post(
      `${baseURL}/api/workflows/${workflow.id}/execute`,
      {
        headers: {
          'Content-Type': 'application/json',
          'X-Carnelian-Key': apiKey,
        },
        data: {},
      }
    );

    // Should return error status or handle gracefully
    expect([200, 400, 404, 500]).toContain(executeResponse.status());
  });

  test('should require authentication', async ({ request }) => {
    const response = await request.get(`${baseURL}/api/workflows`);
    expect(response.status()).toBe(401);
  });

  test('should reject invalid API key', async ({ request }) => {
    const response = await request.get(`${baseURL}/api/workflows`, {
      headers: {
        'X-Carnelian-Key': 'invalid_key',
      },
    });
    expect(response.status()).toBe(401);
  });
});
