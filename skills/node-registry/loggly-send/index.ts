import type { SkillContext, SkillResult } from '../../types';

interface LogglySendParams {
  message: string;
  tags?: string[];
  timestamp?: number;
  level?: string;
}

export async function execute(
  context: SkillContext,
  params: LogglySendParams
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
    const response = await gateway.call('loggly.send', {
      message: params.message,
      tags: params.tags || [],
      timestamp: params.timestamp || Date.now(),
      level: params.level || 'info',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send to Loggly',
    };
  }
}
