import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface RetryExecuteParams {
  operation: string;
  params: Record<string, unknown>;
  maxRetries?: number;
  retryDelay?: number;
  backoffMultiplier?: number;
}

export async function execute(
  context: SkillContext,
  params: RetryExecuteParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.operation) {
    return {
      success: false,
      error: 'operation is required',
    };
  }

  try {
    const response = await gateway.call('retry.execute', {
      operation: params.operation,
      params: params.params || {},
      maxRetries: params.maxRetries || 3,
      retryDelay: params.retryDelay || 1000,
      backoffMultiplier: params.backoffMultiplier || 2,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute with retry',
    };
  }
}
