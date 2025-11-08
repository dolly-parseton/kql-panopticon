#!/usr/bin/env python3
"""
Generate KQL Panopticon query packs from Sentinel-Queries repository.

This script walks through the Sentinel-Queries repository, extracts KQL queries
and their descriptions from .kql files, and generates query pack YAML files
organized by directory.
"""

import os
import sys
from pathlib import Path
from typing import List, Dict, Optional
import yaml
import re


def extract_description_and_query(kql_file_path: Path) -> tuple[Optional[str], str]:
    """
    Extract description from '//' comments and the query content.

    Reads until the first non-empty, non-'//' prefixed line.
    Returns: (description, query_content)
    """
    with open(kql_file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    description_lines = []
    query_start_idx = 0

    # Read lines until we find the first non-comment, non-empty line
    for i, line in enumerate(lines):
        stripped = line.strip()

        # Skip empty lines
        if not stripped:
            continue

        # If it's a comment line, extract the description
        if stripped.startswith('//'):
            # Remove '//' prefix and strip whitespace
            desc_line = stripped[2:].strip()
            if desc_line:  # Only add non-empty description lines
                description_lines.append(desc_line)
        else:
            # Found the first non-comment line - this is where the query starts
            query_start_idx = i
            break

    # Join description lines
    description = ' '.join(description_lines) if description_lines else None

    # Get the full query content (from first non-comment line to end)
    query_content = ''.join(lines[query_start_idx:]).strip()

    return description, query_content


def sanitize_pack_name(folder_name: str) -> str:
    """Convert folder name to a valid pack filename."""
    # Replace spaces and special chars with hyphens
    sanitized = re.sub(r'[^a-zA-Z0-9-_]', '-', folder_name)
    # Convert to lowercase
    sanitized = sanitized.lower()
    # Remove multiple consecutive hyphens
    sanitized = re.sub(r'-+', '-', sanitized)
    # Remove leading/trailing hyphens
    sanitized = sanitized.strip('-')
    return sanitized


def generate_query_pack(folder_name: str, queries: List[Dict]) -> Dict:
    """
    Generate a query pack structure from folder and queries.

    Returns a dict that will be serialized to YAML.
    """
    pack = {
        'name': folder_name,
        'description': f'Queries from {folder_name} category',
        'author': 'reprise99/Sentinel-Queries',
    }

    if len(queries) == 1:
        # Single query format
        pack['query'] = queries[0]['query']
        if queries[0]['description']:
            pack['description'] = queries[0]['description']
    else:
        # Multiple queries format
        pack['queries'] = []
        for q in queries:
            query_entry = {
                'name': q['name'],
                'query': q['query']
            }
            if q['description']:
                query_entry['description'] = q['description']
            pack['queries'].append(query_entry)

    return pack


def process_directory(sentinel_queries_path: Path, output_dir: Path):
    """
    Walk through Sentinel-Queries repo and generate query packs.
    """
    if not sentinel_queries_path.exists():
        print(f"Error: Sentinel-Queries path not found: {sentinel_queries_path}")
        print("Have you added it as a submodule?")
        sys.exit(1)

    # Create output directory if it doesn't exist
    output_dir.mkdir(parents=True, exist_ok=True)

    # Track packs generated
    packs_generated = 0
    queries_processed = 0

    # Walk through all subdirectories
    for root, dirs, files in os.walk(sentinel_queries_path):
        kql_files = [f for f in files if f.endswith('.kql')]

        if not kql_files:
            continue

        # Get folder name relative to repo root
        rel_path = Path(root).relative_to(sentinel_queries_path)
        folder_name = str(rel_path) if str(rel_path) != '.' else 'root'

        # Skip certain folders (optional - customize as needed)
        skip_folders = {'Diagrams', 'Workbooks', '.git'}
        if any(skip in folder_name for skip in skip_folders):
            continue

        print(f"Processing folder: {folder_name} ({len(kql_files)} queries)")

        # Process all .kql files in this folder
        queries = []
        for kql_file in sorted(kql_files):
            kql_path = Path(root) / kql_file
            query_name = kql_file.replace('.kql', '')

            try:
                description, query_content = extract_description_and_query(kql_path)

                if not query_content:
                    print(f"  Warning: Empty query in {kql_file}, skipping")
                    continue

                queries.append({
                    'name': query_name,
                    'description': description,
                    'query': query_content
                })
                queries_processed += 1

            except Exception as e:
                print(f"  Error processing {kql_file}: {e}")
                continue

        if not queries:
            print(f"  No valid queries found in {folder_name}")
            continue

        # Generate query pack
        pack_data = generate_query_pack(folder_name, queries)

        # Create output filename
        pack_filename = sanitize_pack_name(folder_name) + '.yaml'
        pack_path = output_dir / pack_filename

        # Write YAML file
        with open(pack_path, 'w', encoding='utf-8') as f:
            yaml.dump(pack_data, f, default_flow_style=False, sort_keys=False, allow_unicode=True)

        print(f"  Generated: {pack_filename}")
        packs_generated += 1

    print(f"\nSummary:")
    print(f"  Packs generated: {packs_generated}")
    print(f"  Queries processed: {queries_processed}")
    print(f"  Output directory: {output_dir}")


def main():
    """Main entry point."""
    # Default paths (relative to this script)
    script_dir = Path(__file__).parent
    sentinel_queries_path = script_dir / 'Sentinel-Queries'
    output_dir = script_dir / 'generated-packs'

    # Allow override via command line
    if len(sys.argv) > 1:
        sentinel_queries_path = Path(sys.argv[1])
    if len(sys.argv) > 2:
        output_dir = Path(sys.argv[2])

    print(f"Sentinel-Queries path: {sentinel_queries_path}")
    print(f"Output directory: {output_dir}")
    print()

    process_directory(sentinel_queries_path, output_dir)


if __name__ == '__main__':
    main()
