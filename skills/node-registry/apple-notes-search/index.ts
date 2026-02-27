import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AppleNotesSearchParams {
  query: string;
  folder?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: AppleNotesSearchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.query) {
    return {
      success: false,
      error: 'query is required',
    };
  }

  try {
    const response = await gateway.call('apple.notes.search', {
      query: params.query,
      folder: params.folder,
      limit: params.limit || 50,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search Apple notes',
    };
  }
}
