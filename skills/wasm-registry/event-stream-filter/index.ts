import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { pattern, action = 'filter', targetStream } = context.parameters;

    if (!pattern) {
      return { success: false, error: 'pattern is required' };
    }

    const response = await fetch(`${context.gateway_url}/internal/events/filter`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ pattern, action, targetStream }),
    });

    if (!response.ok) {
      return { success: false, error: `Event stream filter failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to filter event stream' };
  }
}
