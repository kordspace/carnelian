import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GmailReadParams {
  query?: string;
  maxResults?: number;
  labelIds?: string[];
  includeSpam?: boolean;
}

export async function execute(
  context: SkillContext,
  params: GmailReadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('gmail.read', {
      query: params.query || '',
      maxResults: params.maxResults || 10,
      labelIds: params.labelIds || [],
      includeSpam: params.includeSpam || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to read Gmail',
    };
  }
}
