import type { SkillContext, SkillResult } from '../../types';

interface LinkedInPostParams {
  text: string;
  visibility?: 'PUBLIC' | 'CONNECTIONS';
  mediaUrl?: string;
}

export async function execute(
  context: SkillContext,
  params: LinkedInPostParams
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
    const response = await gateway.call('linkedin.post', {
      text: params.text,
      visibility: params.visibility || 'PUBLIC',
      mediaUrl: params.mediaUrl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to LinkedIn',
    };
  }
}
