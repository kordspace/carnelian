import type { SkillContext, SkillResult } from '../../types';

interface HttpRequestParams {
  url: string;
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH' | 'HEAD' | 'OPTIONS';
  headers?: Record<string, string>;
  body?: string | Record<string, unknown>;
  timeout?: number;
  followRedirects?: boolean;
}

export async function execute(
  context: SkillContext,
  params: HttpRequestParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.url) {
    return {
      success: false,
      error: 'url is required',
    };
  }

  try {
    const response = await gateway.call('http.request', {
      url: params.url,
      method: params.method || 'GET',
      headers: params.headers || {},
      body: params.body,
      timeout: params.timeout || 30000,
      followRedirects: params.followRedirects !== false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute HTTP request',
    };
  }
}
