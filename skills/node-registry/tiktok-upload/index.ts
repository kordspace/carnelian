import type { SkillContext, SkillResult } from '../../types';

interface TikTokUploadParams {
  videoUrl: string;
  caption: string;
  privacyLevel?: string;
  disableComments?: boolean;
  disableDuet?: boolean;
}

export async function execute(
  context: SkillContext,
  params: TikTokUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.videoUrl || !params.caption) {
    return {
      success: false,
      error: 'videoUrl and caption are required',
    };
  }

  try {
    const response = await gateway.call('tiktok.upload', {
      videoUrl: params.videoUrl,
      caption: params.caption,
      privacyLevel: params.privacyLevel || 'PUBLIC_TO_EVERYONE',
      disableComments: params.disableComments || false,
      disableDuet: params.disableDuet || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to TikTok',
    };
  }
}
