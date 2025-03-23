#!/usr/bin/env python
import os
import sys
import json
import argparse
import time
import datetime
from scrapy.crawler import CrawlerProcess
from scrapy.utils.project import get_project_settings
from benchmark.spiders.benchmark_spider import BenchmarkSpider

# 添加一个JSON编码器来处理datetime对象
class DateTimeEncoder(json.JSONEncoder):
    def default(self, obj):
        if isinstance(obj, datetime.datetime):
            return obj.isoformat()
        return super(DateTimeEncoder, self).default(obj)

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
    
    # Set the output directory for the pipeline
    if args.output_dir:
        # This will be accessed by the pipeline
        settings.set('OUTPUT_DIR', args.output_dir)
    
    # Create the crawler process
    process = CrawlerProcess(settings)
    
    # Create the crawler and get its stats collector
    crawler = process.create_crawler(BenchmarkSpider)
    
    # Run the spider
    process.crawl(crawler)
    process.start()
    
    # Save the stats to a file if requested
    if args.stats_file:
        with open(args.stats_file, 'w') as f:
            json.dump(crawler.stats.get_stats(), f, cls=DateTimeEncoder)

if __name__ == '__main__':
    main() 