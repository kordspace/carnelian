import type { SkillContext, SkillResult } from '../../types';

interface BlueskyPostParams {
  text: string;
  images?: string[];
  replyTo?: string;
}

export async function execute(
  context: SkillContext,
  params: BlueskyPostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.text) {
    return {
      success: false,
      error: 'text is required',
    };
  }

  try {
    const response = await gateway.call('bluesky.post', {
      text: params.text,
      images: params.images || [],
      replyTo: params.replyTo,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Bluesky',
    };
  }
}
