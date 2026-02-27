import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface EmbyLibraryParams {
  userId?: string;
  parentId?: string;
  sortBy?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: EmbyLibraryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('emby.library', {
      userId: params.userId,
      parentId: params.parentId,
      sortBy: params.sortBy || 'SortName',
      limit: params.limit || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Emby library',
    };
  }
}
