pub mod claude_code;
pub mod codex;
pub mod hermes;
pub mod openclaw;
pub mod opencode;

pub fn supported_adapters() -> Vec<&'static str> {
    vec![
        claude_code::ADAPTER_NAME,
        codex::ADAPTER_NAME,
        hermes::ADAPTER_NAME,
        openclaw::ADAPTER_NAME,
        opencode::ADAPTER_NAME,
    ]
}
