import type { SkillContext, SkillResult } from '../../types';

interface TwilioSendSMSParams {
  to: string;
  from: string;
  body: string;
  mediaUrl?: string;
}

export async function execute(
  context: SkillContext,
  params: TwilioSendSMSParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.to || !params.from || !params.body) {
    return {
      success: false,
      error: 'to, from, and body are required',
    };
  }

  try {
    const response = await gateway.call('twilio.sendSMS', {
      to: params.to,
      from: params.from,
      body: params.body,
      mediaUrl: params.mediaUrl,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Twilio SMS',
    };
  }
}
