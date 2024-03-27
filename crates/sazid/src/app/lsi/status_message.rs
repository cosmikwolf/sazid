use helix_core::diagnostic::Severity;
use std::borrow::Cow;

#[derive(Debug, Default)]
pub struct StatusMessage {
  pub msg: Option<(Cow<'static, str>, Severity)>,
}

impl StatusMessage {
  #[inline]
  pub fn clear_status(&mut self) {
    self.msg = None;
  }

  #[inline]
  pub fn set_status<T: Into<Cow<'static, str>>>(&mut self, status: T) {
    let status = status.into();
    log::debug!("editor status: {}", status);
    self.msg = Some((status, Severity::Info));
  }

  #[inline]
  pub fn set_error<T: Into<Cow<'static, str>>>(&mut self, error: T) {
    let error = error.into();
    log::debug!("editor error: {}", error);
    self.msg = Some((error, Severity::Error));
  }

  #[inline]
  pub fn get_status(&self) -> Option<(&Cow<'static, str>, &Severity)> {
    if let Some((status, severity)) = &self.msg {
      Some((status, severity))
    } else {
      None
    }
  }
}
