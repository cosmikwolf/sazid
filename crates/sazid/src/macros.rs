//! These are macros to make getting very nested fields in the `session` struct easier
//! These are macros instead of functions because functions will have to take `&mut self`
//! However, rust doesn't know that you only want a partial borrow instead of borrowing the
//! entire struct which `&mut self` says.  This makes it impossible to do other mutable
//! stuff to the struct because it is already borrowed. Because macros are expanded,
//! this circumvents the problem because it is just like indexing fields by hand and then
//! putting a `&mut` in front of it. This way rust can see that we are only borrowing a
//! part of the struct and not the entire thing.

/// Get the current view and document mutably as a tuple.
/// Returns `(&mut View, &mut Document)`
#[macro_export]
macro_rules! current {
  ($session:expr) => {{
    let view = $crate::view_mut!($session);
    let id = view.doc;
    let doc = $crate::doc_mut!($session, &id);
    (view, doc)
  }};
}

#[macro_export]
macro_rules! current_ref {
  ($session:expr) => {{
    let view = $session.tree.get($session.tree.focus);
    let doc = &$session.documents[&view.doc];
    (view, doc)
  }};
}

/// Get the current document mutably.
/// Returns `&mut Document`
#[macro_export]
macro_rules! doc_mut {
  ($session:expr, $id:expr) => {{
    $session.documents.get_mut($id).unwrap()
  }};
  ($session:expr) => {{
    $crate::current!($session).1
  }};
}

/// Get the current view mutably.
/// Returns `&mut View`
#[macro_export]
macro_rules! view_mut {
  ($session:expr, $id:expr) => {{
    $session.tree.get_mut($id)
  }};
  ($session:expr) => {{
    $session.tree.get_mut($session.tree.focus)
  }};
}

/// Get the current view immutably
/// Returns `&View`
#[macro_export]
macro_rules! view {
  ($session:expr, $id:expr) => {{
    $session.tree.get($id)
  }};
  ($session:expr) => {{
    $session.tree.get($session.tree.focus)
  }};
}

#[macro_export]
macro_rules! doc {
  ($session:expr, $id:expr) => {{
    &$session.documents[$id]
  }};
  ($session:expr) => {{
    $crate::current_ref!($session).1
  }};
}
