import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WordPressCreatePostParams {
  title: string;
  content: string;
  status?: 'publish' | 'draft' | 'pending';
  categories?: number[];
  tags?: number[];
}

export async function execute(
  context: SkillContext,
  params: WordPressCreatePostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.content) {
    return {
      success: false,
      error: 'title and content are required',
    };
  }

  try {
    const response = await gateway.call('wordpress.createPost', {
      title: params.title,
      content: params.content,
      status: params.status || 'draft',
      categories: params.categories || [],
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create WordPress post',
    };
  }
}
