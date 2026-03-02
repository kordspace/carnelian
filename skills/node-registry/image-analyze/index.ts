import type { SkillContext, SkillResult } from '../../types';

interface ImageAnalyzeParams {
  imageUrl?: string;
  imagePath?: string;
  prompt?: string;
  model?: string;
  maxTokens?: number;
}

export async function execute(
  context: SkillContext,
  params: ImageAnalyzeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.imageUrl && !params.imagePath) {
    return {
      success: false,
      error: 'Either imageUrl or imagePath is required',
    };
  }

  try {
    const response = await gateway.call('image.analyze', {
      imageUrl: params.imageUrl,
      imagePath: params.imagePath,
      prompt: params.prompt || 'Describe this image in detail.',
      model: params.model || 'gpt-4-vision-preview',
      maxTokens: params.maxTokens || 500,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to analyze image',
    };
  }
}
