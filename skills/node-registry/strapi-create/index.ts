import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface StrapiCreateParams {
  contentType: string;
  data: Record<string, any>;
  locale?: string;
}

export async function execute(
  context: SkillContext,
  params: StrapiCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.contentType || !params.data) {
    return {
      success: false,
      error: 'contentType and data are required',
    };
  }

  try {
    const response = await gateway.call('strapi.create', {
      contentType: params.contentType,
      data: params.data,
      locale: params.locale,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Strapi content',
    };
  }
}
