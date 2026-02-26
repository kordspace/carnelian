import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GatewayConfigParams {
  action?: 'get' | 'apply';
  config?: Record<string, unknown>;
}

export async function execute(
  context: SkillContext,
  params: GatewayConfigParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  const action = params.action || 'get';

  try {
    if (action === 'get') {
      const response = await gateway.call('gateway.config.get', {});
      return {
        success: true,
        data: response,
      };
    } else if (action === 'apply') {
      if (!params.config || typeof params.config !== 'object') {
        return {
          success: false,
          error: 'config object is required for apply action',
        };
      }

      const response = await gateway.call('gateway.config.apply', params.config);
      return {
        success: true,
        data: response,
      };
    } else {
      return {
        success: false,
        error: `Unknown action: ${action}`,
      };
    }
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to manage gateway config',
    };
  }
}
