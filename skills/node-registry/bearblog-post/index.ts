import type { SkillContext, SkillResult } from '../../types';

interface BearBlogPostParams {
  title: string;
  content: string;
  slug?: string;
  published?: boolean;
}

export async function execute(
  context: SkillContext,
  params: BearBlogPostParams
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
    const response = await gateway.call('bearblog.post', {
      title: params.title,
      content: params.content,
      slug: params.slug,
      published: params.published !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Bear Blog post',
    };
  }
}
