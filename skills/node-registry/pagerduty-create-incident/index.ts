import type { SkillContext, SkillResult } from '../../types';

interface PagerDutyCreateIncidentParams {
  title: string;
  serviceId: string;
  urgency?: 'high' | 'low';
  body?: string;
}

export async function execute(
  context: SkillContext,
  params: PagerDutyCreateIncidentParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.serviceId) {
    return {
      success: false,
      error: 'title and serviceId are required',
    };
  }

  try {
    const response = await gateway.call('pagerduty.createIncident', {
      title: params.title,
      serviceId: params.serviceId,
      urgency: params.urgency || 'high',
      body: params.body,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create PagerDuty incident',
    };
  }
}
