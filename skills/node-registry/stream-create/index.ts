import type { SkillContext, SkillResult } from '../../types';

interface StreamCreateParams {
  name: string;
  type?: 'readable' | 'writable' | 'duplex';
  encoding?: string;
}

export async function execute(
  context: SkillContext,
  params: StreamCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name) {
    return {
      success: false,
      error: 'name is required',
    };
  }

  try {
    const response = await gateway.call('stream.create', {
      name: params.name,
      type: params.type || 'readable',
      encoding: params.encoding || 'utf8',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create stream',
    };
  }
}
