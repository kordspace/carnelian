import type { SkillContext, SkillResult } from '../../types';

interface SendGridSendEmailParams {
  to: string | string[];
  from: string;
  subject: string;
  html: string;
  text?: string;
}

export async function execute(
  context: SkillContext,
  params: SendGridSendEmailParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.to || !params.from || !params.subject || !params.html) {
    return {
      success: false,
      error: 'to, from, subject, and html are required',
    };
  }

  try {
    const response = await gateway.call('sendgrid.sendEmail', {
      to: Array.isArray(params.to) ? params.to : [params.to],
      from: params.from,
      subject: params.subject,
      html: params.html,
      text: params.text,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send SendGrid email',
    };
  }
}
