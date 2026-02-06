use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use quick_xml::events::Event;
use quick_xml::Reader;
use wesichain_core::{Document, Value};

use crate::error::IngestionError;

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

pub async fn load_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| IngestionError::MissingExtension { path: path.clone() })?;

    match extension.as_str() {
        "txt" => load_text_file_async(path).await,
        "docx" => load_docx_file_async(path).await,
        _ => Err(IngestionError::UnsupportedExtension { path, extension }),
    }
}

pub async fn load_files_async(paths: Vec<PathBuf>) -> Result<Vec<Document>, IngestionError> {
    let mut documents = Vec::new();
    for path in paths {
        documents.extend(load_file_async(path).await?);
    }
    Ok(documents)
}

async fn load_text_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|source| IngestionError::Read {
            path: path.clone(),
            source,
        })?;

    let mut metadata = HashMap::new();
    metadata.insert(
        "source".to_string(),
        Value::String(path.to_string_lossy().to_string()),
    );

    Ok(vec![Document {
        id: path.to_string_lossy().to_string(),
        content,
        metadata,
        embedding: None,
    }])
}

async fn load_docx_file_async(path: PathBuf) -> Result<Vec<Document>, IngestionError> {
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|source| IngestionError::Read {
            path: path.clone(),
            source,
        })?;

    let content = parse_docx_text(&bytes).map_err(|source| IngestionError::Parse {
        path: path.clone(),
        source,
    })?;

    let mut metadata = HashMap::new();
    metadata.insert(
        "source".to_string(),
        Value::String(path.to_string_lossy().to_string()),
    );

    Ok(vec![Document {
        id: path.to_string_lossy().to_string(),
        content,
        metadata,
        embedding: None,
    }])
}

fn parse_docx_text(bytes: &[u8]) -> Result<String, std::io::Error> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(invalid_docx)?;

    let mut document_xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(invalid_docx)?
        .read_to_string(&mut document_xml)
        .map_err(invalid_docx)?;

    extract_docx_plain_text(&document_xml)
}

fn extract_docx_plain_text(xml: &str) -> Result<String, std::io::Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    let mut cell = String::new();
    let mut row = Vec::new();
    let mut table_rows = Vec::new();
    let mut inside_table = false;
    let mut inside_paragraph = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match local_name(element.name().as_ref()) {
                b"tbl" => {
                    inside_table = true;
                    table_rows.clear();
                }
                b"tr" if inside_table => {
                    row.clear();
                }
                b"tc" if inside_table => {
                    cell.clear();
                }
                b"p" => {
                    inside_paragraph = true;
                    paragraph.clear();
                }
                _ => {}
            },
            Ok(Event::Text(text)) => {
                if inside_paragraph {
                    let value = text
                        .decode()
                        .map_err(|error| invalid_docx(format!("text decode error: {error}")))?;
                    paragraph.push_str(&value);
                }
            }
            Ok(Event::CData(text)) => {
                if inside_paragraph {
                    let value = text
                        .decode()
                        .map_err(|error| invalid_docx(format!("cdata decode error: {error}")))?;
                    paragraph.push_str(&value);
                }
            }
            Ok(Event::End(element)) => match local_name(element.name().as_ref()) {
                b"p" => {
                    let value = normalize_whitespace(&paragraph);
                    if !value.is_empty() {
                        if inside_table {
                            if !cell.is_empty() {
                                cell.push(' ');
                            }
                            cell.push_str(&value);
                        } else {
                            blocks.push(value);
                        }
                    }
                    inside_paragraph = false;
                    paragraph.clear();
                }
                b"tc" if inside_table => {
                    row.push(cell.trim().to_string());
                    cell.clear();
                }
                b"tr" if inside_table => {
                    if !row.is_empty() {
                        table_rows.push(std::mem::take(&mut row));
                    }
                }
                b"tbl" if inside_table => {
                    let table_text = format_table_rows(&table_rows);
                    if !table_text.is_empty() {
                        blocks.push(table_text);
                    }
                    table_rows.clear();
                    inside_table = false;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(invalid_docx(format!("xml parse error: {error}")));
            }
            _ => {}
        }
    }

    Ok(blocks.join("\n\n"))
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn format_table_rows(rows: &[Vec<String>]) -> String {
    rows.iter()
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .map(|row| row.join(" | "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|byte| *byte == b':') {
        Some(index) => &name[index + 1..],
        None => name,
    }
}

fn invalid_docx<E: std::fmt::Display>(error: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("invalid docx: {error}"))
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
        Err(std::io::Error::other("pdf feature disabled"))
    }
}
