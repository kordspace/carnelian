import type { SkillContext, SkillResult } from '../../types';

interface NotificationSendParams {
  title: string;
  message: string;
  channel?: string;
  priority?: 'low' | 'normal' | 'high' | 'urgent';
  tags?: string[];
  actions?: Array<{
    label: string;
    action: string;
  }>;
}

export async function execute(
  context: SkillContext,
  params: NotificationSendParams
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
    const response = await gateway.call('notification.send', {
      title: params.title,
      message: params.message,
      channel: params.channel,
      priority: params.priority || 'normal',
      tags: params.tags || [],
      actions: params.actions || [],
      timestamp: new Date().toISOString(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send notification',
    };
  }
}
