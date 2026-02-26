import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WebFetchParams {
  url: string;
  extractContent?: boolean;
  includeHtml?: boolean;
  timeout?: number;
  userAgent?: string;
}

export async function execute(
  context: SkillContext,
  params: WebFetchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.url) {
    return {
      success: false,
      error: 'url is required',
    };
  }

  try {
    const response = await gateway.call('web.fetch', {
      url: params.url,
      extractContent: params.extractContent !== false,
      includeHtml: params.includeHtml || false,
      timeout: params.timeout || 30000,
      userAgent: params.userAgent,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to fetch web page',
    };
  }
}
