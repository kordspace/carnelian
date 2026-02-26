import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface CloudflarePurgeCacheParams {
  zoneId: string;
  files?: string[];
  purgeEverything?: boolean;
}

export async function execute(
  context: SkillContext,
  params: CloudflarePurgeCacheParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.zoneId) {
    return {
      success: false,
      error: 'zoneId is required',
    };
  }

  try {
    const response = await gateway.call('cloudflare.purgeCache', {
      zoneId: params.zoneId,
      files: params.files,
      purgeEverything: params.purgeEverything || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to purge Cloudflare cache',
    };
  }
}
