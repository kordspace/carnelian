import type { SkillContext, SkillResult } from '../../types';

interface UniFiNetworkInfoParams {
  siteId?: string;
  type?: 'clients' | 'devices' | 'networks' | 'stats';
}

export async function execute(
  context: SkillContext,
  params: UniFiNetworkInfoParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  try {
    const response = await gateway.call('unifi.network.info', {
      siteId: params.siteId || 'default',
      type: params.type || 'clients',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to get UniFi network info',
    };
  }
}
