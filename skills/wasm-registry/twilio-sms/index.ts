import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface TwilioSMSParams {
  action: 'send' | 'list' | 'get';
  to?: string;
  from?: string;
  body?: string;
  messageSid?: string;
  limit?: number;
}

export async function execute(
  context: SkillContext,
  params: TwilioSMSParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.action) {
    return {
      success: false,
      error: 'action is required',
    };
  }

  try {
    const response = await gateway.call('twilio.sms', {
      action: params.action,
      to: params.to,
      from: params.from,
      body: params.body,
      messageSid: params.messageSid,
      limit: params.limit || 20,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute Twilio SMS action',
    };
  }
}
