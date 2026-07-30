#!/usr/bin/env python3
"""
PointSav Digital Systems | service-message-courier
Agnostic Headless Execution Engine
"""

import os
import sys
import importlib.util
import argparse
import datetime

def load_private_adapter(adapter_name):
    """
    Dynamically loads an execution adapter from the quarantined directory.
    This enforces mathematical isolation between the engine and the physical egress payload.
    """
    adapter_path = os.path.join(os.path.dirname(__file__), 'private-adapters', f"{adapter_name}.py")
    
    if not os.path.exists(adapter_path):
        print(f"[{datetime.datetime.now().isoformat()}] FATAL: Private adapter '{adapter_name}' not found.")
        print("Ensure the Customer has injected the operational script into the /private-adapters/ directory.")
        sys.exit(1)

    spec = importlib.util.spec_from_file_location(adapter_name, adapter_path)
    adapter_module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(adapter_module)
    return adapter_module

def main():
    parser = argparse.ArgumentParser(description="PointSav Headless Execution Engine")
    parser.add_argument("--adapter", required=True, help="Name of the private adapter to execute (without .py)")
    parser.add_argument("--limit", type=int, default=10, help="Operational cap for the execution cycle")
    args = parser.parse_args()

    print(f"[{datetime.datetime.now().isoformat()}] SYSTEM: Initializing service-message-courier...")
    
    # Securely mount the requested operational adapter
    adapter = load_private_adapter(args.adapter)
    
    print(f"[{datetime.datetime.now().isoformat()}] SYSTEM: Adapter '{args.adapter}' mounted successfully.")
    print(f"[{datetime.datetime.now().isoformat()}] SYSTEM: Transferring execution authority to adapter payload...\n")

    # Execute the adapter's primary routine
    try:
        adapter.execute_payload(limit=args.limit)
        print(f"\n[{datetime.datetime.now().isoformat()}] SYSTEM: Execution cycle completed successfully.")
    except Exception as e:
        print(f"\n[{datetime.datetime.now().isoformat()}] SYSTEM ERR: Adapter execution failed. {str(e)}")
        sys.exit(1)

if __name__ == "__main__":
    main()
