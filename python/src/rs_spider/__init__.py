"""RS-Spider Python Bindings

This package provides Python bindings for the RS-Spider web crawler,
a high-performance web crawler written in Rust.
"""

from .rs_spider import (
    PyRequest,
    PyResponse,
    PyItem,
    PySpider,
    PyEngine,
    PyEngineStats,
    PySettings,
    PyEngineConfig,
    PyDownloaderConfig,
    PyDownloader,
    PyScheduler,
)

__version__ = "0.1.0"
__all__ = [
    "PyRequest",
    "PyResponse",
    "PyItem",
    "PySpider",
    "PyEngine",
    "PyEngineStats",
    "PySettings",
    "PyEngineConfig",
    "PyDownloaderConfig",
    "PyDownloader",
    "PyScheduler",
]