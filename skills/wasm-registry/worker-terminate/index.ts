import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { workerId, graceful = true, timeout = 30000 } = context.parameters;

    if (!workerId) {
      return { success: false, error: 'workerId is required' };
    }

    const response = await fetch(`${context.gateway_url}/internal/worker/terminate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workerId, graceful, timeout }),
    });

    if (!response.ok) {
      return { success: false, error: `Worker termination failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to terminate worker' };
  }
}
