import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GeminiComputerUseParams {
  prompt: string;
  action: 'screenshot' | 'click' | 'type' | 'scroll' | 'analyze';
  coordinates?: { x: number; y: number };
  text?: string;
  model?: string;
}

export async function execute(
  context: SkillContext,
  params: GeminiComputerUseParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.prompt || !params.action) {
    return {
      success: false,
      error: 'prompt and action are required',
    };
  }

  try {
    const response = await gateway.call('gemini.computerUse', {
      prompt: params.prompt,
      action: params.action,
      coordinates: params.coordinates,
      text: params.text,
      model: params.model || 'gemini-2.0-flash-exp',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Gemini computer use',
    };
  }
}
