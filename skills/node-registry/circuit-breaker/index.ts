import type { SkillContext, SkillResult } from '../../types';

interface CircuitBreakerParams {
  operation: string;
  params: Record<string, unknown>;
  threshold?: number;
  timeout?: number;
  resetTimeout?: number;
}

export async function execute(
  context: SkillContext,
  params: CircuitBreakerParams
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
    const response = await gateway.call('circuit.execute', {
      operation: params.operation,
      params: params.params || {},
      threshold: params.threshold || 5,
      timeout: params.timeout || 30000,
      resetTimeout: params.resetTimeout || 60000,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute with circuit breaker',
    };
  }
}
