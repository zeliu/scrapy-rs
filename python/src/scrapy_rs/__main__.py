#!/usr/bin/env python
"""
Command-line interface for Scrapy-RS.
"""

import os
import sys
import argparse
import subprocess
from . import (
    SCRAPY_RS_EXECUTABLE, startproject, genspider, crawl, list_spiders, version
)


def main():
    """Main entry point for the scrapyrs command."""
    # search for the rust binary
    if SCRAPY_RS_EXECUTABLE:
        # if found, replace the current process with the rust binary
        cmd = [SCRAPY_RS_EXECUTABLE] + sys.argv[1:]
        try:
            # replace the current process with the rust binary
            os.execv(SCRAPY_RS_EXECUTABLE, cmd)
        except OSError:
            # if execv fails, fallback to subprocess
            sys.exit(subprocess.run(cmd).returncode)
    else:
        # if not found, use python to implement
        parser = argparse.ArgumentParser(
            description="Scrapy-RS: A high-performance web crawler written in Rust"
        )
        subparsers = parser.add_subparsers(dest="command", help="Commands")
        
        # startproject command
        sp_start = subparsers.add_parser(
            "startproject", help="Create a new Scrapy-RS project"
        )
        sp_start.add_argument("name", help="Name of the project")
        sp_start.add_argument("--directory", help="Directory to create the project in")
        
        # genspider command
        sp_gen = subparsers.add_parser("genspider", help="Generate a new spider")
        sp_gen.add_argument("name", help="Name of the spider")
        sp_gen.add_argument("domain", help="Domain to crawl")
        sp_gen.add_argument("--template", help="Template to use")
        
        # crawl command
        sp_crawl = subparsers.add_parser("crawl", help="Run a spider")
        sp_crawl.add_argument("name", help="Name of the spider to run")
        sp_crawl.add_argument("-o", "--output", help="Output file for scraped items")
        sp_crawl.add_argument(
            "--format", default="json", help="Format of the output file"
        )
        sp_crawl.add_argument("-s", "--settings", help="Settings file to use")
        
        # list command
        sp_list = subparsers.add_parser("list", help="List available spiders")
        sp_list.add_argument("-s", "--settings", help="Settings file to use")
        
        # version command
        subparsers.add_parser("version", help="Show version information")
        
        args = parser.parse_args()
        
        if not args.command:
            parser.print_help()
            sys.exit(0)
        
        # 执行对应的命令
        if args.command == "startproject":
            result = startproject(args.name, args.directory)
            sys.exit(result.returncode if hasattr(result, 'returncode') else 0)
        
        elif args.command == "genspider":
            result = genspider(args.name, args.domain, args.template)
            sys.exit(result.returncode if hasattr(result, 'returncode') else 0)
        
        elif args.command == "crawl":
            extra_args = []
            if args.output:
                extra_args.extend(["-o", args.output])
            if args.format:
                extra_args.extend(["--format", args.format])
            if args.settings:
                extra_args.extend(["-s", args.settings])
            
            result = crawl(args.name, *extra_args)
            sys.exit(result.returncode if hasattr(result, 'returncode') else 0)
        
        elif args.command == "list":
            spiders = list_spiders()
            for spider in spiders:
                print(spider)
            sys.exit(0)
        
        elif args.command == "version":
            print(version())
            sys.exit(0)


if __name__ == "__main__":
    main() 