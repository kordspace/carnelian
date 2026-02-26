import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface WebhookSendParams {
  url: string;
  payload: Record<string, unknown>;
  method?: 'POST' | 'PUT' | 'PATCH';
  headers?: Record<string, string>;
  secret?: string;
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: WebhookSendParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.url || !params.payload) {
    return {
      success: false,
      error: 'url and payload are required',
    };
  }

  try {
    const response = await gateway.call('webhook.send', {
      url: params.url,
      payload: params.payload,
      method: params.method || 'POST',
      headers: params.headers || {},
      secret: params.secret,
      timeout: params.timeout || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to send webhook',
    };
  }
}
