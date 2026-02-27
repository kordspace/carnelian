import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TwitterPostParams {
  text: string;
  mediaIds?: string[];
  replyToId?: string;
}

export async function execute(
  context: SkillContext,
  params: TwitterPostParams
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
    const response = await gateway.call('twitter.post', {
      text: params.text,
      mediaIds: params.mediaIds || [],
      replyToId: params.replyToId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Twitter',
    };
  }
}
