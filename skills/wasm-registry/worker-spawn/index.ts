import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { workerType, count = 1, config = {} } = context.parameters;

    if (!workerType) {
      return { success: false, error: 'workerType is required' };
    }

    const response = await fetch(`${context.gateway_url}/internal/worker/spawn`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workerType, count, config }),
    });

    if (!response.ok) {
      return { success: false, error: `Worker spawn failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to spawn worker' };
  }
}
