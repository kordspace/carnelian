import type { SkillContext, SkillResult } from '../../types';

interface SanityCMSCreateParams {
  documentType: string;
  document: Record<string, any>;
  projectId?: string;
  dataset?: string;
}

export async function execute(
  context: SkillContext,
  params: SanityCMSCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.documentType || !params.document) {
    return {
      success: false,
      error: 'documentType and document are required',
    };
  }

  try {
    const response = await gateway.call('sanity.create', {
      documentType: params.documentType,
      document: params.document,
      projectId: params.projectId,
      dataset: params.dataset || 'production',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Sanity CMS document',
    };
  }
}
