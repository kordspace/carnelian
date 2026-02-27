import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface ApiCallParams {
  endpoint: string;
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  params?: Record<string, unknown>;
  headers?: Record<string, string>;
  auth?: {
    type: 'bearer' | 'basic' | 'apikey';
    token?: string;
    username?: string;
    password?: string;
    key?: string;
  };
  timeout?: number;
}

export async function execute(
  context: SkillContext,
  params: ApiCallParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.endpoint) {
    return {
      success: false,
      error: 'endpoint is required',
    };
  }

  try {
    const response = await gateway.call('api.call', {
      endpoint: params.endpoint,
      method: params.method || 'GET',
      params: params.params || {},
      headers: params.headers || {},
      auth: params.auth,
      timeout: params.timeout || 30000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute API call',
    };
  }
}
