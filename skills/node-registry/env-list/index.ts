import type { SkillContext, SkillResult } from '../../types';

interface EnvListParams {
  filter?: string;
  includeValues?: boolean;
}

export async function execute(
  context: SkillContext,
  params: EnvListParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('env.list', {
      filter: params.filter,
      includeValues: params.includeValues !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to list environment variables',
    };
  }
}
