import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BrowserNavigateParams {
  url: string;
  profile?: string;
  waitUntil?: 'load' | 'domcontentloaded' | 'networkidle';
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: BrowserNavigateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.url || typeof params.url !== 'string') {
    return {
      success: false,
      error: 'url is required',
    };
  }

  try {
    const response = await gateway.call('browser.navigate', {
      url: params.url,
      profile: params.profile,
      waitUntil: params.waitUntil || 'load',
      timeout: params.timeout || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to navigate browser',
    };
  }
}
