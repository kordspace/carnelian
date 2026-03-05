import type { SkillContext, SkillResult } from '../../types';

interface NewRelicEventParams {
  action: 'post_event' | 'query_events' | 'create_dashboard';
  eventType?: string;
  attributes?: Record<string, unknown>;
  nrqlQuery?: string;
  dashboardName?: string;
  widgets?: unknown[];
}

export async function execute(
  context: SkillContext,
  params: NewRelicEventParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('newrelic.event', {
      action: params.action,
      eventType: params.eventType,
      attributes: params.attributes,
      nrqlQuery: params.nrqlQuery,
      dashboardName: params.dashboardName,
      widgets: params.widgets,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute NewRelic event action' };
  }
}
