import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { timeRange = '1h', metrics = ['cpu', 'memory', 'tasks'], groupBy = 'worker' } = context.parameters;

    const response = await fetch(`${context.gateway_url}/internal/metrics/aggregate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ timeRange, metrics, groupBy }),
    });

    if (!response.ok) {
      return { success: false, error: `Metrics aggregation failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to aggregate metrics' };
  }
}
