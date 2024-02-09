use std::cell::RefCell;
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use lsp_types::{DocumentSymbol, Range, SymbolKind, SymbolTag};
use url::Url;

#[derive(Debug, Clone)]
pub struct SourceSymbol {
  pub name: String,
  pub detail: Option<String>,
  pub kind: SymbolKind,
  pub tags: Option<Vec<SymbolTag>>,
  pub range: Range,
  pub selection_range: Range,
  pub parent: RefCell<Weak<SourceSymbol>>,
  pub children: RefCell<Vec<Rc<SourceSymbol>>>,
  pub uri: Url,
}

impl SourceSymbol {
  pub fn new(
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    tags: Option<Vec<SymbolTag>>,
    range: Range,
    selection_range: Range,
    uri: Url,
  ) -> Rc<Self> {
    Rc::new(SourceSymbol {
      name,
      detail,
      kind,
      tags,
      range,
      selection_range,
      uri,
      parent: RefCell::new(Weak::new()),
      children: RefCell::new(vec![]),
    })
  }

  pub fn add_child(parent: &Rc<Self>, child: Rc<SourceSymbol>) {
    *child.parent.borrow_mut() = Rc::downgrade(parent);
    parent.children.borrow_mut().push(Rc::clone(&child));
  }

  pub fn iter_tree(rc_self: Rc<Self>) -> impl Iterator<Item = Rc<SourceSymbol>> {
    // Initialize state for the iterator: a stack for DFS
    let mut stack: Vec<Rc<SourceSymbol>> = vec![rc_self];

    std::iter::from_fn(move || {
      if let Some(node) = stack.pop() {
        // When visiting a node, add its children to the stack for later visits
        let children = node.children.borrow();
        for child in children.iter().rev() {
          stack.push(Rc::clone(child));
        }
        Some(Rc::clone(&node))
      } else {
        None // When the stack is empty, iteration ends
      }
    })
  }

  pub fn from_document_symbol(doc_sym: &DocumentSymbol, file_uri: &Url, parent: Option<Rc<SourceSymbol>>) -> Rc<Self> {
    let converted = SourceSymbol::new(
      doc_sym.name.clone(),
      doc_sym.detail.clone(),
      doc_sym.kind,
      doc_sym.tags.clone(),
      doc_sym.range,
      doc_sym.selection_range,
      file_uri.clone(),
    );

    if let Some(p) = parent {
      SourceSymbol::add_child(&p, Rc::clone(&converted));
    }

    if let Some(children) = &doc_sym.children {
      for child in children {
        Self::from_document_symbol(child, file_uri, Some(Rc::clone(&converted)));
      }
    }

    converted
  }
}
impl Display for SourceSymbol {
  fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
    let filename = PathBuf::from(self.uri.path());
    let filename = filename.file_name().unwrap().to_str().unwrap();
    write!(f, "{:?} - {:?}: {}", filename, self.kind, self.name)?;
    let childcount = self.children.borrow().len();
    if childcount > 0 {
      write!(f, " ({} child nodes)", childcount)?;
    }
    Ok(())
  }
}
