import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ConfigWriteParams {
  key: string;
  value: unknown;
  namespace?: string;
  encrypted?: boolean;
}

export async function execute(
  context: SkillContext,
  params: ConfigWriteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key || params.value === undefined) {
    return {
      success: false,
      error: 'key and value are required',
    };
  }

  try {
    const response = await gateway.call('config.write', {
      key: params.key,
      value: params.value,
      namespace: params.namespace || 'default',
      encrypted: params.encrypted || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to write configuration',
    };
  }
}
