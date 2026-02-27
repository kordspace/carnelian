import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface JellyfinLibraryParams {
  userId?: string;
  libraryId?: string;
  sortBy?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: JellyfinLibraryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('jellyfin.library', {
      userId: params.userId,
      libraryId: params.libraryId,
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
      error: error instanceof Error ? error.message : 'Failed to fetch Jellyfin library',
    };
  }
}
