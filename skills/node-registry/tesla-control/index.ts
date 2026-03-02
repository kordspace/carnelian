import type { SkillContext, SkillResult } from '../../types';

interface TeslaControlParams {
  vehicleId: string;
  command: string;
  parameters?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: TeslaControlParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.vehicleId || !params.command) {
    return {
      success: false,
      error: 'vehicleId and command are required',
    };
  }

  try {
    const response = await gateway.call('tesla.control', {
      vehicleId: params.vehicleId,
      command: params.command,
      parameters: params.parameters || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to control Tesla vehicle',
    };
  }
}
