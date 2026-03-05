import type { SkillContext, SkillResult } from '../../types';

interface ReadarrBookParams {
  foreignBookId?: string;
  title?: string;
  author?: string;
  qualityProfileId?: number;
  rootFolderPath?: string;
}

export async function execute(
  context: SkillContext,
  params: ReadarrBookParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('readarr.book', {
      foreignBookId: params.foreignBookId,
      title: params.title,
      author: params.author,
      qualityProfileId: params.qualityProfileId,
      rootFolderPath: params.rootFolderPath,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to manage Readarr book',
    };
  }
}
