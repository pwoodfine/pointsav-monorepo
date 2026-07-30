#!/usr/bin/env python3
import os
import json
import urllib.request
import glob

# Physical Directories
TRANSIENT_QUEUE = "../../service-slm/transient-queues/"
DATASET_OUT = "../datasets/training_dataset.jsonl"
API_URL = "http://127.0.0.1:8080/v1/chat/completions"

print("--------------------------------------------------------")
print("[DISTILLATION] Teacher-Student Dataset Generator Active.")

# The mathematically rigid system prompt
SYSTEM_PROMPT = """You are a deterministic data routing engine for Woodfine Management Corp. 
Read the provided email skeleton and extract the metadata into a strict JSON object.
You must assign EXACTLY ONE Archetype, ONE Domain, and ONE Sentiment.
Allowed Archetypes: EXECUTIVE, GUARDIAN, FIDUCIARY, ENVOY, CONSTRUCTOR.
Allowed Domains: CORPORATE, PROJECTS, DOCUMENTATION, EXTERNAL.
Allowed Sentiments: POSITIVE, NEGATIVE, NEUTRAL.
Output ONLY raw JSON. No markdown formatting. No conversational text."""

skeletons = glob.glob(os.path.join(TRANSIENT_QUEUE, "*.txt"))

if not skeletons:
    print("[WARNING] No shattered skeletons found in the transient queue.")
    exit(0)

processed = 0

with open(DATASET_OUT, "a", encoding="utf-8") as outfile:
    for filepath in skeletons:
        filename = os.path.basename(filepath)
        print(f"[SYSTEM] Submitting {filename} to Teacher Model...")
        
        with open(filepath, "r", encoding="utf-8") as f:
            payload_content = f.read()
            
        # Truncate to protect the 1.5B model's context window on limited RAM
        sanitized_text = payload_content[:1500]
        
        data = {
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": sanitized_text}
            ],
            "temperature": 0.0,
            "max_tokens": 150
        }
        
        req = urllib.request.Request(API_URL, data=json.dumps(data).encode("utf-8"), headers={"Content-Type": "application/json"})
        
        try:
            with urllib.request.urlopen(req, timeout=120) as response:
                result = json.loads(response.read().decode("utf-8"))
                output_content = result["choices"][0]["message"]["content"].strip()
                
                # Clean stray markdown if the model hallucinated it
                output_content = output_content.replace("```json", "").replace("```", "").strip()
                
                # Write the Teacher's perfect Q/A pair to the JSONL dataset
                training_pair = {
                    "instruction": sanitized_text,
                    "output": output_content
                }
                
                outfile.write(json.dumps(training_pair) + "\n")
                print(f"  -> [SUCCESS] Teacher extracted routing logic. Appended to dataset.")
                
                # We intentionally DO NOT delete the skeleton here. 
                # This is a training run, not live production routing.
                processed += 1
                
        except Exception as e:
            print(f"  -> [FAULT] Model failed to process: {e}")

print(f"[SUCCESS] Distillation complete. {processed} records appended to training dataset.")
print("--------------------------------------------------------")
