use sazid::file_chunker::chunk_file;
use std::path::Path;

fn main() {
    let path = Path::new("tests/test_pdf/PDF32000_2008.pdf");
    let total_pages = chunk_file(&path, 0).1;

    println!("Total pages in the PDF: {}", total_pages);

    for i in 0..50 {
        let chunk = chunk_file(&path, i).0;
        println!("Content of Page {}: \n{}", i + 1, chunk);
    }
}
