import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BrowserClickParams {
  selector: string;
  profile?: string;
  button?: 'left' | 'right' | 'middle';
  clickCount?: number;
  delay?: number;
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: BrowserClickParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.selector || typeof params.selector !== 'string') {
    return {
      success: false,
      error: 'selector is required',
    };
  }

  try {
    const response = await gateway.call('browser.click', {
      selector: params.selector,
      profile: params.profile,
      button: params.button || 'left',
      clickCount: params.clickCount || 1,
      delay: params.delay || 0,
      timeout: params.timeout || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to click element',
    };
  }
}
