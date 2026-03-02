import type { SkillContext, SkillResult } from '../../types';

interface AppleMailSendParams {
  to: string[];
  subject: string;
  body: string;
  cc?: string[];
  bcc?: string[];
  attachments?: string[];
}

export async function execute(
  context: SkillContext,
  params: AppleMailSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.to || params.to.length === 0 || !params.subject || !params.body) {
    return {
      success: false,
      error: 'to, subject, and body are required',
    };
  }

  try {
    const response = await gateway.call('appleMail.send', {
      to: params.to,
      subject: params.subject,
      body: params.body,
      cc: params.cc || [],
      bcc: params.bcc || [],
      attachments: params.attachments || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Apple Mail',
    };
  }
}
