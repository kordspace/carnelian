import type { SkillContext, SkillResult } from '../../types';

interface ImgixTransformParams {
  url: string;
  width?: number;
  height?: number;
  fit?: 'crop' | 'scale' | 'max';
  format?: 'jpg' | 'png' | 'webp' | 'avif';
}

export async function execute(
  context: SkillContext,
  params: ImgixTransformParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.url) {
    return {
      success: false,
      error: 'url is required',
    };
  }

  try {
    const response = await gateway.call('imgix.transform', {
      url: params.url,
      width: params.width,
      height: params.height,
      fit: params.fit || 'crop',
      format: params.format || 'webp',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to transform image with Imgix',
    };
  }
}
