import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WhatsAppSendParams {
  to: string;
  message: string;
  accountId?: string;
  mediaUrl?: string;
  mediaType?: string;
}

export async function execute(
  context: SkillContext,
  params: WhatsAppSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.to || !params.message) {
    return {
      success: false,
      error: 'to and message are required',
    };
  }

  try {
    const response = await gateway.call('whatsapp.send', {
      to: params.to,
      message: params.message,
      accountId: params.accountId,
      mediaUrl: params.mediaUrl,
      mediaType: params.mediaType,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send WhatsApp message',
    };
  }
}
