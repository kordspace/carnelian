import type { SkillContext, SkillResult } from '../../types';

interface MastodonPostParams {
  status: string;
  visibility?: 'public' | 'unlisted' | 'private' | 'direct';
  mediaIds?: string[];
  inReplyToId?: string;
}

export async function execute(
  context: SkillContext,
  params: MastodonPostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.status) {
    return {
      success: false,
      error: 'status is required',
    };
  }

  try {
    const response = await gateway.call('mastodon.post', {
      status: params.status,
      visibility: params.visibility || 'public',
      mediaIds: params.mediaIds || [],
      inReplyToId: params.inReplyToId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Mastodon',
    };
  }
}
