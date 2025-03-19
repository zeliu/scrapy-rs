#!/bin/bash
# Script to set up and run a Scrapy benchmark test

# Default values
URL="https://example.com"
OUTPUT_DIR="benchmark_results"
MAX_PAGES=10
MAX_DEPTH=2

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --url)
      URL="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --max-pages)
      MAX_PAGES="$2"
      shift 2
      ;;
    --max-depth)
      MAX_DEPTH="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Create a temporary directory
TEMP_DIR=$(mktemp -d)
echo "Created temporary directory: $TEMP_DIR"

# Create the Scrapy project
echo "Creating Scrapy project..."
cd "$TEMP_DIR"
scrapy startproject benchmark

# Copy the template files
echo "Copying template files..."
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Copy the main files
cp "$SCRIPT_DIR/items.py" "$TEMP_DIR/benchmark/benchmark/"
cp "$SCRIPT_DIR/pipelines.py" "$TEMP_DIR/benchmark/benchmark/"
cp "$SCRIPT_DIR/settings.py" "$TEMP_DIR/benchmark/benchmark/"
cp "$SCRIPT_DIR/__init__.py" "$TEMP_DIR/benchmark/benchmark/"

# Create the spiders directory and copy the spider files
mkdir -p "$TEMP_DIR/benchmark/benchmark/spiders"
cp "$SCRIPT_DIR/spiders/__init__.py" "$TEMP_DIR/benchmark/benchmark/spiders/"
cp "$SCRIPT_DIR/spiders/benchmark_spider.py" "$TEMP_DIR/benchmark/benchmark/spiders/"

# Copy the main.py file
cp "$SCRIPT_DIR/main.py" "$TEMP_DIR/"

# Update the spider file with the provided parameters
SPIDER_FILE="$TEMP_DIR/benchmark/benchmark/spiders/benchmark_spider.py"
sed -i '' "s/self.start_urls = \[\]/self.start_urls = ['$URL']/" "$SPIDER_FILE"
sed -i '' "s/self.max_pages = 100/self.max_pages = $MAX_PAGES/" "$SPIDER_FILE"
sed -i '' "s/self.max_depth = 2/self.max_depth = $MAX_DEPTH/" "$SPIDER_FILE"

# Create the output directory
mkdir -p "$OUTPUT_DIR"

# Run the benchmark
echo "Running benchmark..."
cd "$TEMP_DIR"
python main.py --stats-file stats.json --output-dir "$OUTPUT_DIR"

# Display the results
echo "Benchmark completed!"
echo "Results saved to: $OUTPUT_DIR"

# Display the stats
echo "Stats:"
cat stats.json | python -m json.tool

# Display the crawled items
ITEMS_FILE="$OUTPUT_DIR/items_benchmark.json"
if [ -f "$ITEMS_FILE" ]; then
  echo "Crawled items:"
  cat "$ITEMS_FILE" | python -m json.tool
else
  echo "No items were saved."
fi

# Clean up
echo "Cleaning up temporary directory..."
rm -rf "$TEMP_DIR"

echo "Done!" 