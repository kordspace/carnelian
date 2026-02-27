import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OpsGenieAlertParams {
  action: 'create' | 'close' | 'acknowledge' | 'get' | 'list';
  message?: string;
  alias?: string;
  description?: string;
  priority?: 'P1' | 'P2' | 'P3' | 'P4' | 'P5';
  responders?: string[];
  tags?: string[];
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: OpsGenieAlertParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return { success: false, error: 'Gateway connection not available' };
  }

  try {
    const response = await gateway.call('opsgenie.alert', {
      action: params.action,
      message: params.message,
      alias: params.alias,
      description: params.description,
      priority: params.priority,
      responders: params.responders,
      tags: params.tags,
      limit: params.limit || 100,
    });

    return { success: true, data: response };
  } catch (error) {
    return { success: false, error: error instanceof Error ? error.message : 'Failed to execute OpsGenie alert action' };
  }
}
