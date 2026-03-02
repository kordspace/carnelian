import type { SkillContext, SkillResult } from '../../types';

interface MediumPublishParams {
  title: string;
  content: string;
  contentFormat?: 'html' | 'markdown';
  tags?: string[];
  publishStatus?: 'public' | 'draft' | 'unlisted';
}

export async function execute(
  context: SkillContext,
  params: MediumPublishParams
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
    const response = await gateway.call('medium.publish', {
      title: params.title,
      content: params.content,
      contentFormat: params.contentFormat || 'markdown',
      tags: params.tags || [],
      publishStatus: params.publishStatus || 'draft',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to publish to Medium',
    };
  }
}
