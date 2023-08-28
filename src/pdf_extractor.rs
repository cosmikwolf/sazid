use lopdf::Document;
use lopdf::Object;
use std::collections::BTreeMap;
use std::path::Path;
use std::io::{Error, ErrorKind};

pub struct PdfText {
    pub text: BTreeMap<u32, Vec<String>>, // Key is page number
    pub errors: Vec<String>,
}

static IGNORE: &[&str] = &[
    "Length",
    "BBox",
    "FormType",
    "Matrix",
    "Resources",
    "Type",
    "XObject",
    "Subtype",
    "Filter",
    "ColorSpace",
    "Width",
    "Height",
    "BitsPerComponent",
    "Length1",
    "Length2",
    "Length3",
    "PTEX.FileName",
    "PTEX.PageNumber",
    "PTEX.InfoDict",
    "FontDescriptor",
    "ExtGState",
    "Font",
    "MediaBox",
    "Annot",
];

fn filter_func(object_id: (u32, u16), object: &mut Object) -> Option<((u32, u16), Object)> {
    if IGNORE.contains(&object.type_name().unwrap_or_default()) {
        return None;
    }
    if let Ok(d) = object.as_dict_mut() {
        d.remove(b"Font");
        d.remove(b"Resources");
        d.remove(b"Producer");
        d.remove(b"ModDate");
        d.remove(b"Creator");
        d.remove(b"ProcSet");
        d.remove(b"XObject");
        d.remove(b"MediaBox");
        d.remove(b"Annots");
        if d.is_empty() {
            return None;
        }
    }
    Some((object_id, object.to_owned()))
}

impl PdfText {
    /// Extracts text from a given PDF file
    pub fn from_pdf<P: AsRef<Path>>(pdf_path: P) -> Result<Self, Error> {
        let mut pdf_text = Self {
            text: BTreeMap::new(),
            errors: Vec::new(),
        };
        let doc = Document::load_filtered(pdf_path, filter_func).map_err(|e| {
            Error::new(ErrorKind::Other, format!("Failed to load PDF: {}", e))
        })?;       

        let pages = doc.get_pages();
        for &page_num in pages.keys() {
            let text_result = doc.extract_text(&[page_num]);
            match text_result {
                Ok(text) => {
                    let lines = text
                        .split('\n')
                        .map(|s| s.trim_end().to_string())
                        .collect();
                    pdf_text.text.insert(page_num, lines);
                }
                Err(e) => {
                    pdf_text.errors.push(format!("Page {}: {}", page_num, e));
                }
            }
        }
        Ok(pdf_text)
    }

    /// Get text for a specific page number
    pub fn get_page_text(&self, page_num: u32) -> Option<&Vec<String>> {
        self.text.get(&page_num)
    }

    /// Get total number of pages in the PDF
    pub fn total_pages(&self) -> usize {
        self.text.len()
    }
}