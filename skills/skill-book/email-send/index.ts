import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface EmailSendParams {
  to: string | string[];
  subject: string;
  body: string;
  from?: string;
  cc?: string | string[];
  bcc?: string | string[];
  html?: boolean;
  attachments?: Array<{
    filename: string;
    path?: string;
    content?: string;
  }>;
}

export async function execute(
  context: SkillContext,
  params: EmailSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.to || !params.subject || !params.body) {
    return {
      success: false,
      error: 'to, subject, and body are required',
    };
  }

  try {
    const response = await gateway.call('email.send', {
      to: Array.isArray(params.to) ? params.to : [params.to],
      subject: params.subject,
      body: params.body,
      from: params.from,
      cc: params.cc ? (Array.isArray(params.cc) ? params.cc : [params.cc]) : undefined,
      bcc: params.bcc ? (Array.isArray(params.bcc) ? params.bcc : [params.bcc]) : undefined,
      html: params.html || false,
      attachments: params.attachments || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send email',
    };
  }
}
