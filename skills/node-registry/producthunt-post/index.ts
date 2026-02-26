import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ProductHuntPostParams {
  name: string;
  tagline: string;
  description: string;
  url: string;
  topics?: string[];
}

export async function execute(
  context: SkillContext,
  params: ProductHuntPostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name || !params.tagline || !params.description || !params.url) {
    return {
      success: false,
      error: 'name, tagline, description, and url are required',
    };
  }

  try {
    const response = await gateway.call('producthunt.post', {
      name: params.name,
      tagline: params.tagline,
      description: params.description,
      url: params.url,
      topics: params.topics || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Product Hunt',
    };
  }
}
