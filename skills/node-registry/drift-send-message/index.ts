import type { SkillContext, SkillResult } from '../../types';

interface DriftSendMessageParams {
  conversationId: string;
  message: string;
  type?: 'chat' | 'private_note';
}

export async function execute(
  context: SkillContext,
  params: DriftSendMessageParams
): Promise<SkillResult> {

  if (!params.conversationId || !params.message) {
    return {
      success: false,
      error: 'conversationId and message are required',
    };
  }

  try {
    const response = await fetch(`${context.gateway}/internal/drift/send-message`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        conversationId: params.conversationId,
        message: params.message,
        type: params.type || 'chat',
      }),
    });

    if (!response.ok) {
      return {
        success: false,
        error: `Drift message failed: ${response.statusText}`,
      };
    }

    const data = await response.json();

    return {
      success: true,
      data,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send Drift message',
    };
  }
}
