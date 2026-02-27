import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SentryCaptureParams {
  message: string;
  level?: 'fatal' | 'error' | 'warning' | 'info' | 'debug';
  tags?: Record<string, string>;
  extra?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: SentryCaptureParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.message) {
    return {
      success: false,
      error: 'message is required',
    };
  }

  try {
    const response = await gateway.call('sentry.capture', {
      message: params.message,
      level: params.level || 'error',
      tags: params.tags || {},
      extra: params.extra || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to capture Sentry event',
    };
  }
}
