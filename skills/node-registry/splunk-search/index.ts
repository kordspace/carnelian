import type { SkillContext, SkillResult } from '../../types';

interface SplunkSearchParams {
  search: string;
  earliestTime?: string;
  latestTime?: string;
  maxResults?: number;
}

export async function execute(
  context: SkillContext,
  params: SplunkSearchParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.search) {
    return {
      success: false,
      error: 'search is required',
    };
  }

  try {
    const response = await gateway.call('splunk.search', {
      search: params.search,
      earliestTime: params.earliestTime || '-24h',
      latestTime: params.latestTime || 'now',
      maxResults: params.maxResults || 100,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to search Splunk',
    };
  }
}
