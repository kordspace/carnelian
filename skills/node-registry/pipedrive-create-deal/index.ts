import type { SkillContext, SkillResult } from '../../types';

interface PipedriveCreateDealParams {
  title: string;
  value?: number;
  currency?: string;
  personId?: number;
  orgId?: number;
  stageId?: number;
}

export async function execute(
  context: SkillContext,
  params: PipedriveCreateDealParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title) {
    return {
      success: false,
      error: 'title is required',
    };
  }

  try {
    const response = await gateway.call('pipedrive.createDeal', {
      title: params.title,
      value: params.value,
      currency: params.currency || 'USD',
      personId: params.personId,
      orgId: params.orgId,
      stageId: params.stageId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Pipedrive deal',
    };
  }
}
