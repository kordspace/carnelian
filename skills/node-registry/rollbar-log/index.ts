import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface RollbarLogParams {
  level: 'critical' | 'error' | 'warning' | 'info' | 'debug';
  message: string;
  custom?: Record<string, any>;
  request?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: RollbarLogParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.level || !params.message) {
    return {
      success: false,
      error: 'level and message are required',
    };
  }

  try {
    const response = await gateway.call('rollbar.log', {
      level: params.level,
      message: params.message,
      custom: params.custom || {},
      request: params.request,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to log to Rollbar',
    };
  }
}
