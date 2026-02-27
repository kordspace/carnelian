import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CanvasRenderParams {
  action: 'present' | 'hide' | 'navigate' | 'eval' | 'snapshot' | 'a2ui_push' | 'a2ui_reset';
  node?: string;
  url?: string;
  javaScript?: string;
  outputFormat?: 'png' | 'jpg' | 'jpeg';
  maxWidth?: number;
  quality?: number;
  delayMs?: number;
}

export async function execute(
  context: SkillContext,
  params: CanvasRenderParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('canvas.control', {
      action: params.action,
      node: params.node,
      url: params.url,
      javaScript: params.javaScript,
      outputFormat: params.outputFormat || 'png',
      maxWidth: params.maxWidth,
      quality: params.quality,
      delayMs: params.delayMs,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control canvas',
    };
  }
}
