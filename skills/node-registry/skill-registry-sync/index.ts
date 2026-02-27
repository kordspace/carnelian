import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { targetWorkers = 'all', forceRefresh = false } = context.parameters;

    const response = await fetch(`${context.gateway_url}/internal/skills/sync`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ targetWorkers, forceRefresh }),
    });

    if (!response.ok) {
      return { success: false, error: `Skill registry sync failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to sync skill registry' };
  }
}
