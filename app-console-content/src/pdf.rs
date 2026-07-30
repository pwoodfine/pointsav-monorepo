use image::DynamicImage;
#[cfg(feature = "pdf")]
use pdfium_render::prelude::*;

pub struct PdfPageData {
    pub page: u32,
    pub total_pages: u32,
    pub image: DynamicImage,
}

/// Render `page_idx` (0-based) of the PDF at `path` to a DynamicImage.
/// Requires libpdfium to be installed on the system.
/// Renders at approximately A4 screen resolution (1240px wide).
/// When compiled without the `pdf` feature, always returns an error.
pub fn render_page(path: &str, page_idx: u32) -> anyhow::Result<PdfPageData> {
    #[cfg(not(feature = "pdf"))]
    {
        let _ = (path, page_idx);
        anyhow::bail!("PDF rendering not available in this build");
    }

    #[cfg(feature = "pdf")]
    {
        let bindings = Pdfium::bind_to_system_library()
            .map_err(|e| anyhow::anyhow!("pdfium library not found: {e}"))?;
        let pdfium = Pdfium::new(bindings);

        let document = pdfium
            .load_pdf_from_file(path, None)
            .map_err(|e| anyhow::anyhow!("load PDF: {e}"))?;

        let total_pages = document.pages().len() as u32;
        if page_idx >= total_pages {
            anyhow::bail!(
                "page {} out of range (PDF has {} pages)",
                page_idx,
                total_pages
            );
        }

        let page = document
            .pages()
            .get(page_idx as u16)
            .map_err(|e| anyhow::anyhow!("get page {page_idx}: {e}"))?;

        let render_config = PdfRenderConfig::new()
            .set_target_width(1240)
            .set_maximum_height(1754);

        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| anyhow::anyhow!("render page: {e}"))?;

        let image = bitmap.as_image();

        Ok(PdfPageData {
            page: page_idx,
            total_pages,
            image,
        })
    }
}
