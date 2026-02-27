import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ImageGenerateParams {
  prompt: string;
  model?: string;
  size?: string;
  quality?: string;
  style?: string;
  n?: number;
}

export async function execute(
  context: SkillContext,
  params: ImageGenerateParams
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
    const response = await gateway.call('image.generate', {
      prompt: params.prompt,
      model: params.model || 'dall-e-3',
      size: params.size || '1024x1024',
      quality: params.quality || 'standard',
      style: params.style,
      n: params.n || 1,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate image',
    };
  }
}
