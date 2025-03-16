#!/usr/bin/env python3
import os
from setuptools import setup, find_packages

setup(
    name="scrapy-rs",
    version="0.1.0",
    description="Python bindings for the Scrapy-RS web crawler",
    author="Ze Liu",
    author_email="liuze0518@gmail.com",
    url="https://github.com/liuze/scrapy-rs",
    packages=find_packages("src"),
    package_dir={"": "src"},
    include_package_data=True,
    python_requires=">=3.9",
    install_requires=[],
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Programming Language :: Rust",
        "Topic :: Internet :: WWW/HTTP",
        "Topic :: Software Development :: Libraries :: Python Modules",
    ],
) 