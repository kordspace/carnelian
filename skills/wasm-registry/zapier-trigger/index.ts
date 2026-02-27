import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ZapierTriggerParams {
  webhookUrl: string;
  data: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: ZapierTriggerParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.webhookUrl || !params.data) {
    return {
      success: false,
      error: 'webhookUrl and data are required',
    };
  }

  try {
    const response = await gateway.call('zapier.trigger', {
      webhookUrl: params.webhookUrl,
      data: params.data,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to trigger Zapier webhook',
    };
  }
}
