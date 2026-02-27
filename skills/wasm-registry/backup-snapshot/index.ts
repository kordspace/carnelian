import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { includeDatabase = true, includeLedger = true, includeConfig = true, destination } = context.parameters;

    const response = await fetch(`${context.gateway_url}/internal/backup/snapshot`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ includeDatabase, includeLedger, includeConfig, destination }),
    });

    if (!response.ok) {
      return { success: false, error: `Backup snapshot failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to create backup snapshot' };
  }
}
