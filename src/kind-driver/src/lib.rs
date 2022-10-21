pub mod errors;
pub mod resolution;
pub mod session;

use core::fmt;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;

use kind_report::RenderConfig;
use kind_report::data::{Diagnostic, DiagnosticFrame};
use kind_report::render::FileCache;
use kind_span::SyntaxCtxIndex;
use session::Session;

impl FileCache for Session {
    fn fetch(&self, ctx: SyntaxCtxIndex) -> Option<(Rc<PathBuf>, Rc<String>)> {
        Some((
            self.loaded_paths[ctx.0].clone(),
            self.loaded_sources[ctx.0].clone(),
        ))
    }
}

/// Helper structure to use stderr as fmt::Write
struct ToWriteFmt<T>(pub T);

impl<T> fmt::Write for ToWriteFmt<T>
where
    T: io::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }
}

pub fn render_error_to_stderr(session: &Session, render_config: &RenderConfig, err: &DiagnosticFrame) {
    Diagnostic::render(
        &Diagnostic { frame: err },
        session,
        render_config,
        &mut ToWriteFmt(std::io::stderr()),
    )
    .unwrap();
}
