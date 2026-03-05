import type { SkillContext, SkillResult } from '../../types';

interface MyFitnessPalDiaryParams {
  date?: string;
  username?: string;
}

export async function execute(
  context: SkillContext,
  params: MyFitnessPalDiaryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('myfitnesspal.diary', {
      date: params.date,
      username: params.username,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch MyFitnessPal diary',
    };
  }
}
