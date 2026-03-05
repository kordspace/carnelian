import type { SkillContext, SkillResult } from '../../types';

interface AppleHealthQueryParams {
  dataType: string;
  startDate?: string;
  endDate?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: AppleHealthQueryParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.dataType) {
    return {
      success: false,
      error: 'dataType is required',
    };
  }

  try {
    const response = await gateway.call('applehealth.query', {
      dataType: params.dataType,
      startDate: params.startDate,
      endDate: params.endDate,
      limit: params.limit || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to query Apple Health data',
    };
  }
}
