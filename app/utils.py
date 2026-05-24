from __future__ import annotations

from datetime import datetime, timezone
from typing import Iterable, TypeVar


T = TypeVar("T")


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def paginate(items: list[T], limit: int = 20, offset: int = 0) -> tuple[list[T], int]:
    safe_limit = max(1, min(limit, 100))
    safe_offset = max(0, offset)
    return items[safe_offset : safe_offset + safe_limit], len(items)


def today_count(timestamps: Iterable[str], now: str | None = None) -> int:
    current_day = (now or utc_now_iso())[:10]
    return sum(1 for value in timestamps if value[:10] == current_day)
