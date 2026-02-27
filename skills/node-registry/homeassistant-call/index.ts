import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface HomeAssistantCallParams {
  domain: string;
  service: string;
  entityId?: string;
  data?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: HomeAssistantCallParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.domain || !params.service) {
    return {
      success: false,
      error: 'domain and service are required',
    };
  }

  try {
    const response = await gateway.call('homeassistant.call', {
      domain: params.domain,
      service: params.service,
      entityId: params.entityId,
      data: params.data || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to call Home Assistant service',
    };
  }
}
