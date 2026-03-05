import type { SkillContext, SkillResult } from '../../types';

interface BugsnagNotifyParams {
  errorClass: string;
  errorMessage: string;
  severity?: 'error' | 'warning' | 'info';
  context?: string;
  metadata?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: BugsnagNotifyParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.errorClass || !params.errorMessage) {
    return {
      success: false,
      error: 'errorClass and errorMessage are required',
    };
  }

  try {
    const response = await gateway.call('bugsnag.notify', {
      errorClass: params.errorClass,
      errorMessage: params.errorMessage,
      severity: params.severity || 'error',
      context: params.context,
      metadata: params.metadata || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to notify Bugsnag',
    };
  }
}
