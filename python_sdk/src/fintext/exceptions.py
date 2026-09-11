"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Exceptions
═══════════════════════════════════════════════════════════════════════════════
"""

from typing import Any, Optional


class FinTextError(Exception):
    """Base exception for all FinText SDK errors."""

    def __init__(self, message: str) -> None:
        super().__init__(message)
        self.message = message


class FinTextConnectionError(FinTextError):
    """Raised when communication with the FinText API server fails."""

    pass


class FinTextValidationError(FinTextError):
    """Raised when client-side parameter validation fails."""

    pass


class FinTextAPIError(FinTextError):
    """Raised when the FinText API returns a non-2xx HTTP status code."""

    def __init__(
        self,
        status_code: int,
        error: str,
        message: str,
        response_body: Optional[Any] = None,
        headers: Optional[dict[str, str]] = None,
    ) -> None:
        super().__init__(f"[{status_code}] {error}: {message}")
        self.status_code = status_code
        self.error = error
        self.message = message
        self.response_body = response_body
        self.headers = headers or {}


class FinTextAuthError(FinTextAPIError):
    """Raised when authentication or authorization fails (HTTP 401/403)."""

    pass


class FinTextRateLimitError(FinTextAPIError):
    """
    Raised when the API request exceeds the allocated token bucket quota (HTTP 429).
    """

    def __init__(
        self,
        status_code: int,
        error: str,
        message: str,
        retry_after: Optional[int] = None,
        limit: Optional[int] = None,
        remaining: Optional[int] = None,
        reset: Optional[int] = None,
        response_body: Optional[Any] = None,
        headers: Optional[dict[str, str]] = None,
    ) -> None:
        super().__init__(status_code, error, message, response_body, headers)
        self.retry_after = retry_after
        self.limit = limit
        self.remaining = remaining
        self.reset = reset

    def __str__(self) -> str:
        retry_str = f" (Retry after {self.retry_after}s)" if self.retry_after is not None else ""
        return f"[HTTP 429] Rate limit exceeded{retry_str}: {self.message}"
