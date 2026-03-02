import type { SkillContext, SkillResult } from '../../types';

interface LockAcquireParams {
  resource: string;
  timeout?: number;
  ttl?: number;
}

export async function execute(
  context: SkillContext,
  params: LockAcquireParams
): Promise<SkillResult> {
  if (!params.resource) {
    return {
      success: false,
      error: 'resource is required',
    };
  }

  try {
    const response = await fetch(`${context.gateway}/internal/lock/acquire`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        resource: params.resource,
        timeout: params.timeout || 5000,
        ttl: params.ttl || 30000,
      }),
    });

    if (!response.ok) {
      return {
        success: false,
        error: `Failed to acquire lock: ${response.statusText}`,
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
      error: error instanceof Error ? error.message : 'Failed to acquire lock',
    };
  }
}
