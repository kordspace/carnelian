import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ContentfulCreateEntryParams {
  contentType: string;
  fields: Record<string, any>;
  locale?: string;
}

export async function execute(
  context: SkillContext,
  params: ContentfulCreateEntryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.contentType || !params.fields) {
    return {
      success: false,
      error: 'contentType and fields are required',
    };
  }

  try {
    const response = await gateway.call('contentful.createEntry', {
      contentType: params.contentType,
      fields: params.fields,
      locale: params.locale || 'en-US',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Contentful entry',
    };
  }
}
