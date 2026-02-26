import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface OutlookSendParams {
  to: string | string[];
  subject: string;
  body: string;
  cc?: string | string[];
  bcc?: string | string[];
  importance?: 'low' | 'normal' | 'high';
}

export async function execute(
  context: SkillContext,
  params: OutlookSendParams
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
    const response = await gateway.call('outlook.send', {
      to: Array.isArray(params.to) ? params.to : [params.to],
      subject: params.subject,
      body: params.body,
      cc: params.cc ? (Array.isArray(params.cc) ? params.cc : [params.cc]) : [],
      bcc: params.bcc ? (Array.isArray(params.bcc) ? params.bcc : [params.bcc]) : [],
      importance: params.importance || 'normal',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Outlook email',
    };
  }
}
