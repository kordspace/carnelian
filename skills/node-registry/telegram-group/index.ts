import type { SkillContext, SkillResult } from '../../types';

interface TelegramGroupParams {
  action: 'create' | 'info' | 'members' | 'invite' | 'kick';
  accountId?: string;
  chatId?: string;
  title?: string;
  userId?: number;
  userIds?: number[];
}

export async function execute(
  context: SkillContext,
  params: TelegramGroupParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('telegram.group', {
      action: params.action,
      accountId: params.accountId,
      chatId: params.chatId,
      title: params.title,
      userId: params.userId,
      userIds: params.userIds,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to perform Telegram group action',
    };
  }
}
