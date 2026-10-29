import logging
import re
from collections.abc import Callable
from urllib.parse import urlparse

from assistant.interfaces import FollowUp, ToolResult

log = logging.getLogger(__name__)

_SENTENCE_END = re.compile(r"(?<=[.!?])\s+")


class SearchError(Exception):
    """A search failed. str(error) is a sentance that is safe to speak."""


def _speakable(snippet: str, limit: int = 280, max_sentences: int = 2) -> str:
    """Turn a search snippet into something pleasent to hear."""
    text = re.sub(r"https?://\S+", "", snippet)
    text = re.sub(r"[^\w\s.,;:!?'\"%$&()/+-]", " ", text)       # drop emoji and stray symbols
    text = " ".join(text.split())
    text = re.sub(r"\s*\.{2,}\s*$", "", text)                    # trailing "..." truncation
    text = " ".join(_SENTENCE_END.split(text)[:max_sentences])
    if len(text) > limit:
        text = text[:limit].rsplit(" ", 1)[0]
    return text.rstrip(" ,;:-")


class WebSearch:
    def __init__(
        self,
        region: str = "us-en",
        backend: str = "auto", max_results: int = 5,
        timeout: int = 8,
        search_fn: Callable[[str], list[dict]] | None = None
    ) -> None:
        self._region = region
        self._backend = backend
        self._max_results = max_results
        self._timeout = timeout
        self._search = search_fn or self._ddgs_search

    def _ddgs_search(self, query: str) -> list[dict]:
        try:
            from ddgs import DDGS
            from ddgs.exceptions import DDGSException
        except ImportError as e:
            raise SearchError("Web search needs the ddgs package. Install it with pip.") from e
        try:
            return list(DDGS(timeout=self._timeout).text(
                query, region=self._region, max_results=self._max_results,
                backend=self._backend
            ))
        except (DDGSException, OSError) as e:
            if "no results" in str(e).lower():
                return []
            log.warning("web search failed: %s", e)
            raise SearchError("I couldn't search the web right now.") from e

    def web_search(self, slots: dict[str, str]) -> ToolResult:
        query = slots.get("query", "").strip()
        if not query:
            return ToolResult(False, "What should I search for?",
                follow_up=FollowUp("web_search", "query"))
        try:
            raw = self._search(query)
        except SearchError as e:
            return ToolResult(False, str(e))
        results = [{"title": r.get("title", ""), "url": r.get("href", ""),
            "snippet": r.get("body", "")} for r in raw]
        if not results:
            return ToolResult(False, f"I didn't find anything for {query}.")
        best = next((r for r in results if _speakable(r["snippet"])), None)
        if best is None:
            return ToolResult(True, f"The top result is {results[0]['title']}.",
                {"results": results})
        domain = urlparse(best["url"]).netloc.removeprefix("www.")
        intro = f"According to {domain}: " if domain else ""
        return ToolResult(True, intro + _speakable(best["snippet"]), {"results": results})
