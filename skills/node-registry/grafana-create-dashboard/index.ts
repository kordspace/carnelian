import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface GrafanaCreateDashboardParams {
  title: string;
  dashboard: Record<string, any>;
  folderId?: number;
}

export async function execute(
  context: SkillContext,
  params: GrafanaCreateDashboardParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.title || !params.dashboard) {
    return {
      success: false,
      error: 'title and dashboard are required',
    };
  }

  try {
    const response = await gateway.call('grafana.createDashboard', {
      title: params.title,
      dashboard: params.dashboard,
      folderId: params.folderId,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to create Grafana dashboard',
    };
  }
}
