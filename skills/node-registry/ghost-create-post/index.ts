import type { SkillContext, SkillResult } from '../../types';

interface GhostCreatePostParams {
  title: string;
  html: string;
  status?: 'published' | 'draft';
  tags?: string[];
  featured?: boolean;
}

export async function execute(
  context: SkillContext,
  params: GhostCreatePostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.html) {
    return {
      success: false,
      error: 'title and html are required',
    };
  }

  try {
    const response = await gateway.call('ghost.createPost', {
      title: params.title,
      html: params.html,
      status: params.status || 'draft',
      tags: params.tags || [],
      featured: params.featured || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Ghost post',
    };
  }
}
