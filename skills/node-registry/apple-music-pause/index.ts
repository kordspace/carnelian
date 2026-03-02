import type { SkillContext, SkillResult } from '../../types';

export async function execute(
  context: SkillContext,
  params: Record<string, never>
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('apple.music.pause', {});

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to pause Apple Music',
    };
  }
}
