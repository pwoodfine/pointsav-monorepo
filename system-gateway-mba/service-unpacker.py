import os
import json
import base64

# Target the exact WORM drop location
SOURCE_DIR = "/home/mathew/deployments/woodfine-fleet-deployment/cluster-totebox-personnel-1/service-fs/data/service-people/source"

print("[SYSTEM] Scanning WORM drive for encapsulated assets...")

for filename in os.listdir(SOURCE_DIR):
    if filename.endswith(".json"):
        filepath = os.path.join(SOURCE_DIR, filename)
        
        with open(filepath, 'r') as f:
            try:
                payload = json.load(f)
                original_name = payload['file']['filename']
                base64_data = payload['file']['data']
                
                # Strip the "data:mime/type;base64," prefix from the string
                if "," in base64_data:
                    base64_data = base64_data.split(",")[1]
                
                # Decode the math back into physical bytes
                file_bytes = base64.b64decode(base64_data)
                
                # Write the materialized file back to the disk
                output_path = os.path.join(SOURCE_DIR, f"EXTRACTED_{original_name}")
                with open(output_path, 'wb') as out_file:
                    out_file.write(file_bytes)
                    
                print(f"[SUCCESS] Unpacked JSON Envelope: {filename}")
                print(f"   -> Materialized Physical Asset: EXTRACTED_{original_name}")
                
            except Exception as e:
                print(f"[ERROR] Failed to unpack {filename}: {e}")
