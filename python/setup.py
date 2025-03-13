#!/usr/bin/env python3
from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="rs-spider",
    version="0.1.0",
    description="Python bindings for the RS-Spider web crawler",
    long_description=open("README.md").read(),
    long_description_content_type="text/markdown",
    author="Your Name",
    author_email="your.email@example.com",
    url="https://github.com/yourusername/rs-spider",
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.7",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Rust",
        "Topic :: Internet :: WWW/HTTP",
        "Topic :: Software Development :: Libraries :: Python Modules",
    ],
    keywords="web crawler, scraper, spider, rust",
    python_requires=">=3.7",
    rust_extensions=[
        RustExtension(
            "rs_spider",
            path="Cargo.toml",
            binding=Binding.PyO3,
            debug=False,
        )
    ],
    packages=["rs_spider"],
    package_dir={"": "src"},
    include_package_data=True,
    zip_safe=False,
    install_requires=[],
    extras_require={
        "dev": [
            "pytest>=6.0.0",
            "pytest-cov>=2.10.0",
            "black>=20.8b1",
            "isort>=5.0.0",
            "flake8>=3.8.0",
        ],
    },
) 