import os
import re

def migrate(repo_path):
    # Mapping of categories for re-bucketing (manual logic based on folder structure)
    category_map = {
        'topic-bim': 'architecture',
        'topic-co-location': 'research',
        'topic-gis': 'substrate',
        'topic-regional': 'systems',
        'topic-retail': 'services'
    }

    # 1. Rename and Normalize
    for root, _, files in os.walk(repo_path):
        for file in files:
            if file.startswith('topic-'):
                old_path = os.path.join(root, file)
                new_name = file[len('topic-'):]
                new_path = os.path.join(root, new_name)
                os.rename(old_path, new_path)
                print(f"Renamed: {old_path} -> {new_path}")
    
    # 2. Update Wikilinks
    for root, _, files in os.walk(repo_path):
        for file in files:
            if file.endswith('.md'):
                path = os.path.join(root, file)
                with open(path, 'r') as f:
                    content = f.read()
                
                # Replace [[topic-xyz]] with [[xyz]]
                new_content = re.sub(r'\[\[topic-([^\]|]+)\]\]', r'[[\1]]', content)
                
                if new_content != content:
                    with open(path, 'w') as f:
                        f.write(new_content)
                    print(f"Updated links in: {path}")

migrate('content-wiki-projects')
migrate('content-wiki-corporate')
