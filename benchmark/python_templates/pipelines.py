import json
import os

class BenchmarkPipeline:
    """
    Pipeline for saving benchmark items to a JSON file.
    """
    def __init__(self):
        self.items = []
        # This content will be replaced at runtime
        self.output_dir = None
        self.output_file = None
        
    def process_item(self, item, spider):
        self.items.append(dict(item))
        return item
        
    def close_spider(self, spider):
        if self.output_file:
            # Create the output directory if it doesn't exist
            if self.output_dir:
                os.makedirs(self.output_dir, exist_ok=True)
                
            # Write the items to the output file
            with open(self.output_file, 'w') as f:
                json.dump(self.items, f) 