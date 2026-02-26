import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BrowserPdfParams {
  profile?: string;
  format?: 'A4' | 'Letter' | 'Legal';
  landscape?: boolean;
  printBackground?: boolean;
  scale?: number;
  margin?: {
    top?: string;
    right?: string;
    bottom?: string;
    left?: string;
  };
}

export async function execute(
  context: SkillContext,
  params: BrowserPdfParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('browser.pdf', {
      profile: params.profile,
      format: params.format || 'A4',
      landscape: params.landscape ?? false,
      printBackground: params.printBackground ?? true,
      scale: params.scale || 1,
      margin: params.margin,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate PDF',
    };
  }
}
