import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ZulipSendParams {
  type: 'stream' | 'private';
  to: string | string[];
  topic?: string;
  content: string;
}

export async function execute(
  context: SkillContext,
  params: ZulipSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.type || !params.to || !params.content) {
    return {
      success: false,
      error: 'type, to, and content are required',
    };
  }

  try {
    const response = await gateway.call('zulip.send', {
      type: params.type,
      to: params.to,
      topic: params.topic,
      content: params.content,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Zulip message',
    };
  }
}
