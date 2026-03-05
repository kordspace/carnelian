import type { SkillContext, SkillResult } from '../../types';

interface WhoopRecoveryParams {
  startDate?: string;
  endDate?: string;
}

export async function execute(
  context: SkillContext,
  params: WhoopRecoveryParams
): Promise<SkillResult> {

  try {
    const response = await fetch(`${context.gateway}/internal/whoop/recovery`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        startDate: params.startDate,
        endDate: params.endDate,
      }),
    });

    if (!response.ok) {
      return {
        success: false,
        error: `Whoop recovery fetch failed: ${response.statusText}`,
      };
    }

    const data = await response.json();

    return {
      success: true,
      data,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch WHOOP recovery data',
    };
  }
}
