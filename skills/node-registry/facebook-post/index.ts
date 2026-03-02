import type { SkillContext, SkillResult } from '../../types';

interface FacebookPostParams {
  message: string;
  pageId?: string;
  link?: string;
  imageUrl?: string;
}

export async function execute(
  context: SkillContext,
  params: FacebookPostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.message) {
    return {
      success: false,
      error: 'message is required',
    };
  }

  try {
    const response = await gateway.call('facebook.post', {
      message: params.message,
      pageId: params.pageId,
      link: params.link,
      imageUrl: params.imageUrl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Facebook',
    };
  }
}
