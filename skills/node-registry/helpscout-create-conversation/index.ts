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

  if (!params.subject || !params.customerEmail || !params.mailboxId || !params.message) {
    return {
      success: false,
      error: 'subject, customerEmail, mailboxId, and message are required',
    };
  }

  try {
    const response = await fetch(`${context.gateway}/internal/helpscout/create-conversation`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        subject: params.subject,
        customerEmail: params.customerEmail,
        mailboxId: params.mailboxId,
        message: params.message,
        type: params.type || 'email',
      }),
    });

    if (!response.ok) {
      return {
        success: false,
        error: `HelpScout conversation creation failed: ${response.statusText}`,
      };
    }

    const data = await response.json();

    return {
      success: true,
      data,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Help Scout conversation',
    };
  }
}
