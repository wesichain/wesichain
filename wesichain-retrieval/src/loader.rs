use std::path::Path;

pub struct TextLoader;

impl TextLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn load<P: AsRef<Path>>(&self, path: P) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfLoaderError {
    FeatureDisabled,
    ExtractFailed(String),
}

pub struct PdfLoader;

impl PdfLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn load<P: AsRef<Path>>(&self, path: P) -> Result<String, PdfLoaderError> {
        self.load_internal(path)
    }

    #[cfg(feature = "pdf")]
    fn load_internal<P: AsRef<Path>>(&self, path: P) -> Result<String, PdfLoaderError> {
        pdf_extract::extract_text(path)
            .map_err(|err| PdfLoaderError::ExtractFailed(err.to_string()))
    }

    #[cfg(not(feature = "pdf"))]
    fn load_internal<P: AsRef<Path>>(&self, _path: P) -> Result<String, PdfLoaderError> {
        Err(PdfLoaderError::FeatureDisabled)
    }
}
