import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { configPath, validateOnly = false } = context.parameters;

    const response = await fetch(`${context.gateway_url}/internal/config/reload`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ configPath, validateOnly }),
    });

    if (!response.ok) {
      return { success: false, error: `Config reload failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to reload configuration' };
  }
}
