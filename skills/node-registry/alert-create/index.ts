import type { SkillContext, SkillResult } from '../../types';

interface AlertCreateParams {
  name: string;
  condition: string;
  threshold: number;
  severity?: 'info' | 'warning' | 'error' | 'critical';
  actions?: Array<{
    type: string;
    config: Record<string, unknown>;
  }>;
}

export async function execute(
  context: SkillContext,
  params: AlertCreateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.name || !params.condition || params.threshold === undefined) {
    return {
      success: false,
      error: 'name, condition, and threshold are required',
    };
  }

  try {
    const response = await gateway.call('alert.create', {
      name: params.name,
      condition: params.condition,
      threshold: params.threshold,
      severity: params.severity || 'warning',
      actions: params.actions || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create alert',
    };
  }
}
