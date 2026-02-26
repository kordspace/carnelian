import type { SkillContext, SkillResult } from '../../../workers/node-worker/src/types';

interface PdfGenerateParams {
  html?: string;
  url?: string;
  markdown?: string;
  outputPath?: string;
  format?: 'A4' | 'Letter' | 'Legal';
  landscape?: boolean;
  margin?: {
    top?: string;
    right?: string;
    bottom?: string;
    left?: string;
  };
  headerTemplate?: string;
  footerTemplate?: string;
  displayHeaderFooter?: boolean;
}

export async function execute(
  context: SkillContext,
  params: PdfGenerateParams
): Promise<SkillResult> {
  const { gateway } = context;

  if (!gateway) {
    return {
      success: false,
      error: 'Gateway connection not available',
    };
  }

  if (!params.html && !params.url && !params.markdown) {
    return {
      success: false,
      error: 'Either html, url, or markdown is required',
    };
  }

  try {
    const response = await gateway.call('pdf.generate', {
      html: params.html,
      url: params.url,
      markdown: params.markdown,
      outputPath: params.outputPath,
      format: params.format || 'A4',
      landscape: params.landscape || false,
      margin: params.margin || { top: '1cm', right: '1cm', bottom: '1cm', left: '1cm' },
      headerTemplate: params.headerTemplate,
      footerTemplate: params.footerTemplate,
      displayHeaderFooter: params.displayHeaderFooter || false,
    });

    return {
      success: true,
      data: response,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to generate PDF',
    };
  }
}
