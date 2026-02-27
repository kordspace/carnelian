import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface AlexaSkillInvokeParams {
  skillId: string;
  intent: string;
  slots?: Record<string, string>;
  deviceId?: string;
}

export async function execute(
  context: SkillContext,
  params: AlexaSkillInvokeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.skillId || !params.intent) {
    return {
      success: false,
      error: 'skillId and intent are required',
    };
  }

  try {
    const response = await gateway.call('alexa.invoke', {
      skillId: params.skillId,
      intent: params.intent,
      slots: params.slots || {},
      deviceId: params.deviceId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to invoke Alexa skill',
    };
  }
}
