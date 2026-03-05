import type { SkillContext, SkillResult } from '../../types';

interface TraceLogParams {
  message: string;
  level?: 'trace' | 'debug' | 'info' | 'warn' | 'error';
  context?: Record<string, unknown>;
  traceId?: string;
}

export async function execute(
  context: SkillContext,
  params: TraceLogParams
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
    const response = await gateway.call('trace.log', {
      message: params.message,
      level: params.level || 'info',
      context: params.context || {},
      traceId: params.traceId,
      timestamp: new Date().toISOString(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to log trace',
    };
  }
}
