import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PagerDutyIncidentParams {
  action: 'create' | 'acknowledge' | 'resolve' | 'get' | 'list';
  serviceId?: string;
  incidentId?: string;
  title?: string;
  description?: string;
  urgency?: 'high' | 'low';
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: PagerDutyIncidentParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('pagerduty.incident', {
      action: params.action,
      serviceId: params.serviceId,
      incidentId: params.incidentId,
      title: params.title,
      description: params.description,
      urgency: params.urgency,
      limit: params.limit || 25,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute PagerDuty incident action' };
  }
}
