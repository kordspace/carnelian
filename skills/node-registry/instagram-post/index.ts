import type { SkillContext, SkillResult } from '../../types';

interface InstagramPostParams {
  imageUrl: string;
  caption?: string;
  location?: string;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: InstagramPostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.imageUrl) {
    return {
      success: false,
      error: 'imageUrl is required',
    };
  }

  try {
    const response = await gateway.call('instagram.post', {
      imageUrl: params.imageUrl,
      caption: params.caption,
      location: params.location,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Instagram',
    };
  }
}
