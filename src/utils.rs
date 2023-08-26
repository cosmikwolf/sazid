use std::fs;
use std::path::PathBuf;
use std::io::{self, Read};

pub fn determine_file_type(file_path: &PathBuf) -> Result<String, io::Error> {
    let extension = file_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match extension {
        "txt" => Ok("text".to_string()),
        "pdf" => Ok("pdf".to_string()),
        "html" => Ok("html".to_string()),
        "rs" => Ok("rust".to_string()),
        "toml" => Ok("toml".to_string()),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported file type")),
    }
}

pub fn read_file(file_path: &PathBuf) -> Result<String, io::Error> {
    let mut file = fs::File::open(file_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_determine_file_type() {
        let txt_path = PathBuf::from("test.txt");
        let pdf_path = PathBuf::from("test.pdf");
        let html_path = PathBuf::from("test.html");
        let rs_path = PathBuf::from("test.rs");
        let toml_path = PathBuf::from("test.toml");
        let unknown_path = PathBuf::from("test.unknown");

        File::create(&txt_path).unwrap();
        File::create(&pdf_path).unwrap();
        File::create(&html_path).unwrap();
        File::create(&rs_path).unwrap();
        File::create(&toml_path).unwrap();
        File::create(&unknown_path).unwrap();

        assert_eq!(determine_file_type(&txt_path).unwrap(), "text");
        assert_eq!(determine_file_type(&pdf_path).unwrap(), "pdf");
        assert_eq!(determine_file_type(&html_path).unwrap(), "html");
        assert_eq!(determine_file_type(&rs_path).unwrap(), "rust");
        assert_eq!(determine_file_type(&toml_path).unwrap(), "toml");
        assert!(determine_file_type(&unknown_path).is_err());

        fs::remove_file(txt_path).unwrap();
        fs::remove_file(pdf_path).unwrap();
        fs::remove_file(html_path).unwrap();
        fs::remove_file(rs_path).unwrap();
        fs::remove_file(toml_path).unwrap();
        fs::remove_file(unknown_path).unwrap();
    }

    #[test]
    fn test_read_file() {
        let txt_path = PathBuf::from("test_read.txt");
        let content = "Hello, world!";
        fs::write(&txt_path, content).unwrap();

        let read_content = read_file(&txt_path).unwrap();
        assert_eq!(read_content, content);

        fs::remove_file(txt_path).unwrap();
    }
}
