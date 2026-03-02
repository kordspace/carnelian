import type { SkillContext, SkillResult } from '../../types';

interface HelpScoutCreateConversationParams {
  subject: string;
  customerEmail: string;
  mailboxId: number;
  message: string;
  type?: 'email' | 'chat' | 'phone';
}

export async function execute(
  context: SkillContext,
  params: HelpScoutCreateConversationParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.subject || !params.customerEmail || !params.mailboxId || !params.message) {
    return {
      success: false,
      error: 'subject, customerEmail, mailboxId, and message are required',
    };
  }

  try {
    const response = await gateway.call('helpscout.createConversation', {
      subject: params.subject,
      customerEmail: params.customerEmail,
      mailboxId: params.mailboxId,
      message: params.message,
      type: params.type || 'email',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Help Scout conversation',
    };
  }
}
