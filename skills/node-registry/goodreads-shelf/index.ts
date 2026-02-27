import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GoodreadsShelfParams {
  userId: string;
  shelf?: string;
  page?: number;
  perPage?: number;
}

export async function execute(
  context: SkillContext,
  params: GoodreadsShelfParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.userId) {
    return {
      success: false,
      error: 'userId is required',
    };
  }

  try {
    const response = await gateway.call('goodreads.shelf', {
      userId: params.userId,
      shelf: params.shelf || 'to-read',
      page: params.page || 1,
      perPage: params.perPage || 20,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch Goodreads shelf',
    };
  }
}
