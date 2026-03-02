import type { SkillContext, SkillResult } from '../../types';

interface NestThermostatParams {
  deviceId: string;
  targetTemperature?: number;
  mode?: string;
  fanMode?: string;
}

export async function execute(
  context: SkillContext,
  params: NestThermostatParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.deviceId) {
    return {
      success: false,
      error: 'deviceId is required',
    };
  }

  try {
    const response = await gateway.call('nest.thermostat', {
      deviceId: params.deviceId,
      targetTemperature: params.targetTemperature,
      mode: params.mode,
      fanMode: params.fanMode,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Nest thermostat',
    };
  }
}
