import type { SkillContext, SkillResult } from '../../types';

interface EcobeeControlParams {
  thermostatId: string;
  temperature?: number;
  hvacMode?: string;
  holdType?: string;
}

export async function execute(
  context: SkillContext,
  params: EcobeeControlParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.thermostatId) {
    return {
      success: false,
      error: 'thermostatId is required',
    };
  }

  try {
    const response = await gateway.call('ecobee.control', {
      thermostatId: params.thermostatId,
      temperature: params.temperature,
      hvacMode: params.hvacMode,
      holdType: params.holdType || 'nextTransition',
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Ecobee thermostat',
    };
  }
}
