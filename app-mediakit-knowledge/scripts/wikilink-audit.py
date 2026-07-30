import os
import re

# Simple audit script to verify wikilink resolution across the content corpus.
# Scans all Markdown files for [[slug]] patterns and checks existence.

def run_audit(root_dir):
    all_slugs = set()
    # Collect all valid file paths (slugs)
    for root, _, files in os.walk(root_dir):
        for file in files:
            if file.endswith('.md'):
                all_slugs.add(os.path.splitext(file)[0])
    
    broken_links = []
    wikilink_re = re.compile(r'\[\[([^\]|]+)(?:\|[^\]]+)?\]\]')

    for root, _, files in os.walk(root_dir):
        for file in files:
            if file.endswith('.md'):
                with open(os.path.join(root, file), 'r') as f:
                    content = f.read()
                    matches = wikilink_re.findall(content)
                    for slug in matches:
                        if slug not in all_slugs:
                            broken_links.append((file, slug))
    
    return broken_links

broken = run_audit('.')
if broken:
    print(f"Audit failed! Found {len(broken)} broken links:")
    for f, s in broken:
        print(f" - {f}: [[{s}]]")
else:
    print("Audit passed: All wikilinks resolve.")
