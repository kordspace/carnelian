import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface BackupCreateParams {
  source: string;
  destination?: string;
  compress?: boolean;
  incremental?: boolean;
}

export async function execute(
  context: SkillContext,
  params: BackupCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.source) {
    return {
      success: false,
      error: 'source is required',
    };
  }

  try {
    const response = await gateway.call('backup.create', {
      source: params.source,
      destination: params.destination,
      compress: params.compress !== false,
      incremental: params.incremental || false,
      timestamp: Date.now(),
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create backup',
    };
  }
}
