import type { SkillContext, SkillResult } from '../../types';

interface TailscaleStatusParams {
  action?: 'status' | 'peers' | 'routes';
}

export async function execute(
  context: SkillContext,
  params: TailscaleStatusParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('tailscale.status', {
      action: params.action || 'status',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get Tailscale status',
    };
  }
}
