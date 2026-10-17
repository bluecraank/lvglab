//! AI assistant of LVGLab: describes the project to Claude, asks for a UI
//! design in a fixed JSON schema and applies the answer to the project.
//!
//! * [`prompt`] builds the cached system prompt (widget catalog reference)
//!   and the per-request context.
//! * [`client`] talks to the Claude Messages API.
//! * [`spec`] / [`convert`] define the exchanged format and turn it into
//!   validated widgets.

pub mod client;
pub mod convert;
pub mod prompt;
pub mod spec;

pub use client::{AiError, Answer, Config, EFFORTS, MODELS, Usage};
pub use convert::{Applied, apply};
pub use prompt::Scope;
pub use spec::Design;

/// Runs one request: builds the prompt for `scope`, calls the model and
/// returns its design (not yet applied).
pub fn request(
    config: &Config,
    project: &lvglab_model::Project,
    scope: Scope,
    text: &str,
    on_progress: impl FnMut(usize),
) -> Result<Answer, AiError> {
    let system = prompt::system_prompt();
    let user = prompt::user_message(project, scope, text);
    client::generate(config, &system, &user, on_progress)
}
