import type { SkillContext, SkillResult } from '../../types';

interface DiscordCreateChannelParams {
  guildId: string;
  name: string;
  type?: 'text' | 'voice' | 'category';
  topic?: string;
}

export async function execute(
  context: SkillContext,
  params: DiscordCreateChannelParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.guildId || !params.name) {
    return {
      success: false,
      error: 'guildId and name are required',
    };
  }

  try {
    const response = await gateway.call('discord.createChannel', {
      guildId: params.guildId,
      name: params.name,
      type: params.type || 'text',
      topic: params.topic,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Discord channel',
    };
  }
}
