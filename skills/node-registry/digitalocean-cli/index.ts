import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface DigitalOceanCLIParams {
  command: string;
  resource?: 'droplet' | 'database' | 'kubernetes' | 'app';
  action?: string;
  args?: Record<string, any>;
}

export async function execute(
  context: SkillContext,
  params: DigitalOceanCLIParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.command) {
    return {
      success: false,
      error: 'command is required',
    };
  }

  try {
    const response = await gateway.call('digitalocean.cli', {
      command: params.command,
      resource: params.resource,
      action: params.action,
      args: params.args || {},
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to execute DigitalOcean CLI command',
    };
  }
}
