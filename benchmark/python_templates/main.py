#!/usr/bin/env python
import os
import sys
import json
import argparse
import time
from scrapy.crawler import CrawlerProcess
from scrapy.utils.project import get_project_settings
from benchmark.spiders.benchmark_spider import BenchmarkSpider

def main():
    # Parse command line arguments
    parser = argparse.ArgumentParser(description='Run Scrapy benchmark')
    parser.add_argument('--stats-file', type=str, help='Path to save stats')
    parser.add_argument('--output-dir', type=str, help='Directory to save output')
    args = parser.parse_args()
    
    # Get the project settings
    settings = get_project_settings()
    
    # Add stats collector settings
    if args.stats_file:
        settings.set('STATS_DUMP', True)
    
    # Create the crawler process
    process = CrawlerProcess(settings)
    
    # Configure the spider
    spider = BenchmarkSpider()
    
    # Set the output directory for the pipeline
    if args.output_dir:
        # This will be accessed by the pipeline
        settings.set('OUTPUT_DIR', args.output_dir)
    
    # Run the spider
    process.crawl(spider)
    process.start()
    
    # Save the stats to a file if requested
    if args.stats_file:
        with open(args.stats_file, 'w') as f:
            json.dump(spider.crawler.stats.get_stats(), f)

if __name__ == '__main__':
    main() 