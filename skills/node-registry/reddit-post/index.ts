import type { SkillContext, SkillResult } from '../../types';

interface RedditPostParams {
  subreddit: string;
  title: string;
  text?: string;
  url?: string;
  flair?: string;
}

export async function execute(
  context: SkillContext,
  params: RedditPostParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.subreddit || !params.title) {
    return {
      success: false,
      error: 'subreddit and title are required',
    };
  }

  try {
    const response = await gateway.call('reddit.post', {
      subreddit: params.subreddit,
      title: params.title,
      text: params.text,
      url: params.url,
      flair: params.flair,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to post to Reddit',
    };
  }
}
