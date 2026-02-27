import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface StabilityAIGenerateParams {
  prompt: string;
  negativePrompt?: string;
  width?: number;
  height?: number;
  steps?: number;
  cfgScale?: number;
  seed?: number;
}

export async function execute(
  context: SkillContext,
  params: StabilityAIGenerateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.prompt) {
    return {
      success: false,
      error: 'prompt is required',
    };
  }

  try {
    const response = await gateway.call('stabilityai.generate', {
      prompt: params.prompt,
      negativePrompt: params.negativePrompt,
      width: params.width || 1024,
      height: params.height || 1024,
      steps: params.steps || 30,
      cfgScale: params.cfgScale || 7.0,
      seed: params.seed,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate image with Stability AI',
    };
  }
}
