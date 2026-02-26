import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BrowserTypeParams {
  selector: string;
  text: string;
  profile?: string;
  delay?: number;
  clear?: boolean;
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: BrowserTypeParams
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

  if (!params.text || typeof params.text !== 'string') {
    return {
      success: false,
      error: 'text is required',
    };
  }

  try {
    const response = await gateway.call('browser.type', {
      selector: params.selector,
      text: params.text,
      profile: params.profile,
      delay: params.delay || 0,
      clear: params.clear ?? true,
      timeout: params.timeout || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to type text',
    };
  }
}
