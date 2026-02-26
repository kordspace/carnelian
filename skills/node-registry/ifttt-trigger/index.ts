import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface IFTTTTriggerParams {
  event: string;
  key: string;
  value1?: string;
  value2?: string;
  value3?: string;
}

export async function execute(
  context: SkillContext,
  params: IFTTTTriggerParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.event || !params.key) {
    return {
      success: false,
      error: 'event and key are required',
    };
  }

  try {
    const response = await gateway.call('ifttt.trigger', {
      event: params.event,
      key: params.key,
      value1: params.value1,
      value2: params.value2,
      value3: params.value3,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to trigger IFTTT event',
    };
  }
}
