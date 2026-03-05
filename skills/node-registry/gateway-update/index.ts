import type { SkillContext, SkillResult } from '../../types';

interface GatewayUpdateParams {
  action?: 'check' | 'run';
  autoRestart?: boolean;
}

export async function execute(
  context: SkillContext,
  params: GatewayUpdateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  const action = params.action || 'check';

  try {
    if (action === 'check') {
      const response = await gateway.call('gateway.update.check', {});
      return {
        success: true,
        data: response,
      };
    } else if (action === 'run') {
      const response = await gateway.call('gateway.update.run', {
        autoRestart: params.autoRestart ?? false,
      });
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
      error: error instanceof Error ? error.message : 'Failed to update gateway',
    };
  }
}
