import type { SkillContext, SkillResult } from '../../types';

interface VonageSMSParams {
  to: string;
  from: string;
  text: string;
}

export async function execute(
  context: SkillContext,
  params: VonageSMSParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.to || !params.from || !params.text) {
    return {
      success: false,
      error: 'to, from, and text are required',
    };
  }

  try {
    const response = await gateway.call('vonage.sms', {
      to: params.to,
      from: params.from,
      text: params.text,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Vonage SMS',
    };
  }
}
