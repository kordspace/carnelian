import type { SkillContext, SkillResult } from '../../types';

interface AnthropicClaudeParams {
  model?: string;
  messages: Array<{ role: string; content: string }>;
  maxTokens?: number;
  temperature?: number;
  systemPrompt?: string;
}

export async function execute(
  context: SkillContext,
  params: AnthropicClaudeParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.messages || params.messages.length === 0) {
    return {
      success: false,
      error: 'messages array is required',
    };
  }

  try {
    const response = await gateway.call('anthropic.claude', {
      model: params.model || 'claude-3-5-sonnet-20241022',
      messages: params.messages,
      maxTokens: params.maxTokens || 4096,
      temperature: params.temperature || 1.0,
      systemPrompt: params.systemPrompt,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to call Anthropic Claude API',
    };
  }
}
