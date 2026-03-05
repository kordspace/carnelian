import type { SkillContext, SkillResult } from '../../types';

interface MiroCreateBoardParams {
  name: string;
  description?: string;
  teamId?: string;
}

export async function execute(
  context: SkillContext,
  params: MiroCreateBoardParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('miro.createBoard', {
      name: params.name,
      description: params.description || '',
      teamId: params.teamId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Miro board',
    };
  }
}
