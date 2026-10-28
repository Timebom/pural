from assistant.interfaces import ToolResult


def chat_reply(slots: dict[str, str]) -> ToolResult:
    """Speaks an answer the LLM already wrote (the 'chat_reply' pseudo-intent)."""
    text = slots.get("text", "").strip()
    if not text:
        return ToolResult(False, "Sorry, I don't have an answer for that.")
    return ToolResult(True, text)
