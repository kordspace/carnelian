import { SkillContext, SkillResult } from '../../types';

export async function execute(context: SkillContext): Promise<SkillResult> {
  try {
    const { olderThanDays = 30, keepCheckpoints = true } = context.parameters;

    const response = await fetch(`${context.gateway_url}/internal/ledger/compact`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ olderThanDays, keepCheckpoints }),
    });

    if (!response.ok) {
      return { success: false, error: `Ledger compaction failed: ${response.statusText}` };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to compact ledger' };
  }
}
