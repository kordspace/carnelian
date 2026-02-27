import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { workerType, checkInterval = 5000, thresholds = {} } = context.parameters;

    const response = await fetch(`${context.gateway_url}/internal/worker/health`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workerType, checkInterval, thresholds }),
    });

    if (!response.ok) {
      return { success: false, error: `Worker health check failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to monitor worker health' };
  }
}
