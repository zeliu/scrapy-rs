#!/bin/bash
set -e

# Ensure we are in the correct directory
cd "$(dirname "$0")"

# Clean up old build files
rm -rf target/wheels
mkdir -p target/wheels

# Activate virtual environment (if available)
if [ -d "../venv" ]; then
    source ../venv/bin/activate
fi

# Install necessary tools
pip install -U pip
pip install -U maturin twine

# Build wheel package
echo "Building wheels..."
maturin build --release --out target/wheels

# List built wheel packages
echo "Built wheels:"
ls -la target/wheels

# If --publish parameter is provided, publish to PyPI
if [ "$1" == "--publish" ]; then
    echo "Publishing to PyPI..."
    twine upload target/wheels/*
    echo "Published to PyPI"
fi

echo "Done!" 