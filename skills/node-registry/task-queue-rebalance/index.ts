import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { strategy = 'load-based', targetWorkers, priority = 'normal' } = context.parameters;

    const response = await fetch(`${context.gateway_url}/internal/queue/rebalance`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ strategy, targetWorkers, priority }),
    });

    if (!response.ok) {
      return { success: false, error: `Queue rebalance failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to rebalance task queue' };
  }
}
