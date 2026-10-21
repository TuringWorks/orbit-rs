"""
Shared utility helpers for OrbitRS Python example scripts.

Provides environment-variable helpers that were previously duplicated across
the insurance, telco, and manufacturing example workflows.
"""

import os


def require_env(name):
    """Return the named environment variable or raise a clear error."""
    value = os.environ.get(name)
    if not value:
        raise EnvironmentError(
            f"{name} environment variable must be set. "
            "See README for required environment variables."
        )
    return value


def env_int(name, default):
    """Parse an integer environment variable, raising a clear error on bad input."""
    raw = os.getenv(name, str(default))
    try:
        return int(raw)
    except ValueError:
        raise EnvironmentError(
            f"{name} must be an integer, got '{raw}'. "
            "See README for required environment variables."
        )
