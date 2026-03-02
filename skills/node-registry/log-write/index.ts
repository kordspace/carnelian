import type { SkillContext, SkillResult } from '../../types';

interface LogWriteParams {
  message: string;
  level?: 'debug' | 'info' | 'warn' | 'error';
  context?: Record<string, unknown>;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: LogWriteParams
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
    const response = await gateway.call('log.write', {
      message: params.message,
      level: params.level || 'info',
      context: params.context || {},
      tags: params.tags || [],
      timestamp: new Date().toISOString(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to write log',
    };
  }
}
