use crate::config::Target;
use crate::tools::CodingAgentTool;
use crate::tools::claude::CLAUDE_TOOL;
use crate::tools::codex::CODEX_TOOL;

static TOOLS: [&dyn CodingAgentTool; 2] = [&CODEX_TOOL, &CLAUDE_TOOL];

pub(crate) fn tool_for(target: Target) -> &'static dyn CodingAgentTool {
    all_tools()
        .iter()
        .copied()
        .find(|tool| tool.target() == target)
        .expect("target should have registered tool")
}

pub(crate) fn all_tools() -> &'static [&'static dyn CodingAgentTool] {
    debug_assert!(TOOLS.iter().all(|tool| !tool.id().is_empty()));
    &TOOLS
}

pub(crate) fn tool_sort_key(target: Target) -> usize {
    all_tools()
        .iter()
        .position(|tool| tool.target() == target)
        .expect("target should have registered tool")
}
