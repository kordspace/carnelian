import type { SkillContext, SkillResult } from '../../types';

interface AlertSendParams {
  title: string;
  message: string;
  severity?: 'info' | 'warning' | 'error' | 'critical';
  channels?: string[];
}

export async function execute(
  context: SkillContext,
  params: AlertSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.message) {
    return {
      success: false,
      error: 'title and message are required',
    };
  }

  try {
    const response = await gateway.call('alert.send', {
      title: params.title,
      message: params.message,
      severity: params.severity || 'info',
      channels: params.channels || ['default'],
      timestamp: new Date().toISOString(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send alert',
    };
  }
}
