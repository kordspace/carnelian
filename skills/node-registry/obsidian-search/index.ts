import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ObsidianSearchParams {
  vault: string;
  query: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: ObsidianSearchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.vault || !params.query) {
    return {
      success: false,
      error: 'vault and query are required',
    };
  }

  try {
    const response = await gateway.call('obsidian.search', {
      vault: params.vault,
      query: params.query,
      limit: params.limit || 50,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search Obsidian vault',
    };
  }
}
