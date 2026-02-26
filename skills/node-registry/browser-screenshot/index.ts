import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BrowserScreenshotParams {
  profile?: string;
  fullPage?: boolean;
  format?: 'png' | 'jpeg';
  quality?: number;
  selector?: string;
}

export async function execute(
  context: SkillContext,
  params: BrowserScreenshotParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('browser.screenshot', {
      profile: params.profile,
      fullPage: params.fullPage ?? false,
      format: params.format || 'png',
      quality: params.quality,
      selector: params.selector,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to take screenshot',
    };
  }
}
