import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ConfigReadParams {
  key: string;
  namespace?: string;
  defaultValue?: unknown;
}

export async function execute(
  context: SkillContext,
  params: ConfigReadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.key) {
    return {
      success: false,
      error: 'key is required',
    };
  }

  try {
    const response = await gateway.call('config.read', {
      key: params.key,
      namespace: params.namespace || 'default',
      defaultValue: params.defaultValue,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to read configuration',
    };
  }
}
