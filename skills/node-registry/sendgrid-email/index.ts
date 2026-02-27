import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface SendGridEmailParams {
  to: string;
  from: string;
  subject: string;
  text?: string;
  html?: string;
  templateId?: string;
  dynamicTemplateData?: Record<string, unknown>;
}

export async function execute(
  context: SkillContext,
  params: SendGridEmailParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.to || !params.from || !params.subject) {
    return {
      success: false,
      error: 'to, from, and subject are required',
    };
  }

  try {
    const response = await gateway.call('sendgrid.email', {
      to: params.to,
      from: params.from,
      subject: params.subject,
      text: params.text,
      html: params.html,
      templateId: params.templateId,
      dynamicTemplateData: params.dynamicTemplateData,
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
