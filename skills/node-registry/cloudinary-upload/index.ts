import type { SkillContext, SkillResult } from '../../types';

interface CloudinaryUploadParams {
  file: string;
  folder?: string;
  publicId?: string;
  tags?: string[];
}

export async function execute(
  context: SkillContext,
  params: CloudinaryUploadParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.file) {
    return {
      success: false,
      error: 'file is required',
    };
  }

  try {
    const response = await gateway.call('cloudinary.upload', {
      file: params.file,
      folder: params.folder,
      publicId: params.publicId,
      tags: params.tags || [],
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to upload to Cloudinary',
    };
  }
}
