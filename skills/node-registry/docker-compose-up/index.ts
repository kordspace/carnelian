import type { SkillContext, SkillResult } from '../../types';

interface DockerComposeUpParams {
  projectDirectory: string;
  detached?: boolean;
  build?: boolean;
  services?: string[];
}

export async function execute(
  context: SkillContext,
  params: DockerComposeUpParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.projectDirectory) {
    return {
      success: false,
      error: 'projectDirectory is required',
    };
  }

  try {
    const response = await gateway.call('dockerCompose.up', {
      projectDirectory: params.projectDirectory,
      detached: params.detached !== false,
      build: params.build || false,
      services: params.services || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to run docker-compose up',
    };
  }
}
