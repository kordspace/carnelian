import type { SkillContext, SkillResult } from '../../types';

interface HackerNewsPostParams {
  title: string;
  url?: string;
  text?: string;
}

export async function execute(
  context: SkillContext,
  params: HackerNewsPostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title) {
    return {
      success: false,
      error: 'title is required',
    };
  }

  if (!params.url && !params.text) {
    return {
      success: false,
      error: 'Either url or text must be provided',
    };
  }

  try {
    const response = await gateway.call('hackernews.post', {
      title: params.title,
      url: params.url,
      text: params.text,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Hacker News',
    };
  }
}
