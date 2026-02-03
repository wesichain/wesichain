pub struct TextLoader;

#[cfg(feature = "pdf")]
pub struct PdfLoader;

#[cfg(not(feature = "pdf"))]
pub struct PdfLoader;
