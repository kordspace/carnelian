import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TelegramMediaParams {
  action: 'photo' | 'video' | 'audio' | 'document' | 'sticker';
  accountId?: string;
  chatId: string;
  fileUrl?: string;
  fileId?: string;
  caption?: string;
  parseMode?: 'Markdown' | 'HTML';
}

export async function execute(
  context: SkillContext,
  params: TelegramMediaParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action || !params.chatId) {
    return {
      success: false,
      error: 'action and chatId are required',
    };
  }

  if (!params.fileUrl && !params.fileId) {
    return {
      success: false,
      error: 'Either fileUrl or fileId is required',
    };
  }

  try {
    const response = await gateway.call('telegram.media', {
      action: params.action,
      accountId: params.accountId,
      chatId: params.chatId,
      fileUrl: params.fileUrl,
      fileId: params.fileId,
      caption: params.caption,
      parseMode: params.parseMode,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Telegram media',
    };
  }
}
