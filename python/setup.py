#!/usr/bin/env python3
import os
from setuptools import setup, find_packages

setup(
    name="scrapy-rs",
    version="0.1.0",
    author="Scrapy-RS Team",
    author_email="example@example.com",
    description="Python bindings for Scrapy-RS, a high-performance web crawler written in Rust",
    long_description=open("../README.md").read(),
    long_description_content_type="text/markdown",
    url="https://github.com/yourusername/scrapy-rs",
    packages=find_packages("src"),
    package_dir={"": "src"},
    classifiers=[
        "Programming Language :: Python :: 3",
        "Programming Language :: Rust",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
    ],
    python_requires=">=3.6",
) 