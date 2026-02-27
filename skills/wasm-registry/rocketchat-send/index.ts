import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface RocketChatSendParams {
  channel: string;
  text: string;
  alias?: string;
  emoji?: string;
}

export async function execute(
  context: SkillContext,
  params: RocketChatSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.channel || !params.text) {
    return {
      success: false,
      error: 'channel and text are required',
    };
  }

  try {
    const response = await gateway.call('rocketchat.send', {
      channel: params.channel,
      text: params.text,
      alias: params.alias,
      emoji: params.emoji,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Rocket.Chat message',
    };
  }
}
