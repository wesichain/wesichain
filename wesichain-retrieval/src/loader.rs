use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use wesichain_core::{Document, Value};

pub struct TextLoader {
    path: PathBuf,
}

impl TextLoader {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Vec<Document>, std::io::Error> {
        let content = fs::read_to_string(&self.path)?;
        let mut metadata = HashMap::new();
        metadata.insert(
            "source".to_string(),
            Value::String(self.path.to_string_lossy().to_string()),
        );

        Ok(vec![Document {
            id: self.path.to_string_lossy().to_string(),
            content,
            metadata,
            embedding: None,
        }])
    }
}

#[cfg(feature = "pdf")]
pub struct PdfLoader {
    path: PathBuf,
}

#[cfg(feature = "pdf")]
impl PdfLoader {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Vec<Document>, std::io::Error> {
        let content = pdf_extract::extract_text(&self.path).map_err(std::io::Error::other)?;
        let mut metadata = HashMap::new();
        metadata.insert(
            "source".to_string(),
            Value::String(self.path.to_string_lossy().to_string()),
        );

        Ok(vec![Document {
            id: self.path.to_string_lossy().to_string(),
            content,
            metadata,
            embedding: None,
        }])
    }
}

#[cfg(not(feature = "pdf"))]
pub struct PdfLoader {
    path: PathBuf,
}

#[cfg(not(feature = "pdf"))]
impl PdfLoader {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Vec<Document>, std::io::Error> {
        let _ = &self.path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "pdf feature disabled",
        ))
    }
}
