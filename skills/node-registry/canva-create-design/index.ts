import type { SkillContext, SkillResult } from '../../types';

interface CanvaCreateDesignParams {
  designType: string;
  title?: string;
  width?: number;
  height?: number;
}

export async function execute(
  context: SkillContext,
  params: CanvaCreateDesignParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.designType) {
    return {
      success: false,
      error: 'designType is required',
    };
  }

  try {
    const response = await gateway.call('canva.createDesign', {
      designType: params.designType,
      title: params.title,
      width: params.width,
      height: params.height,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Canva design',
    };
  }
}
