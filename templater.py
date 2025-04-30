#!/usr/bin/env python3
import os
import re
import sys
import argparse
from pathlib import Path
from typing import List, Set

CONDITION_RE = re.compile(r'#if (.+)$')
ENDIF_RE = re.compile(r'#endif\s*$')

def evaluate_condition(condition: str, flags: Set[str]) -> bool:
    condition = condition.strip()
    if condition.startswith('(and '):
        terms = condition[5:-1].split()
        return all(term in flags for term in terms)
    elif condition.startswith('(or '):
        terms = condition[4:-1].split()
        return any(term in flags for term in terms)
    else:
        return condition in flags

def process_lines(lines: List[str], flags: Set[str]) -> List[str]:
    output = []
    include_stack = [True]  # Start with unconditional inclusion

    for line in lines:
        if match := CONDITION_RE.search(line):
            condition = match.group(1)
            include = evaluate_condition(condition, flags)
            include_stack.append(include and include_stack[-1])  # Nesting
            if line.strip() == match.group(0):  # Whole line is just the conditional
                continue
            else:
                # It's a single-line inline conditional
                if include_stack[-1]:
                    output.append(line[:match.start()].rstrip())
                include_stack.pop()
                continue
        elif ENDIF_RE.search(line):
            if len(include_stack) > 1:
                include_stack.pop()
            else:
                raise ValueError("Mismatched #endif without #if")
        else:
            if include_stack[-1]:
                output.append(line.rstrip('\n'))

    if len(include_stack) != 1:
        raise ValueError("Mismatched #if / #endif")

    return output

def process_file(src_path: Path, dest_path: Path, flags: Set[str]):
    with src_path.open() as f:
        lines = f.readlines()

    processed = process_lines(lines, flags)

    # Skip writing if all lines are whitespace or the file is empty
    if all(line.strip() == "" for line in processed):
        return  # Don't write empty/whitespace-only files

    dest_path.parent.mkdir(parents=True, exist_ok=True)
    with dest_path.open('w') as f:
        f.write('\n'.join(processed) + '\n')

def main():
    parser = argparse.ArgumentParser(description="Template processor with conditional blocks.")
    parser.add_argument('--from', dest='src_dir', default='.', help='Source directory with templates')
    parser.add_argument('--to', dest='dest_dir', default='.', help='Destination directory')
    parser.add_argument('flags', nargs='+', help='Flags like `clj devshell` to include conditionals')
    args = parser.parse_args()

    src_dir = Path(args.src_dir)
    dest_dir = Path(args.dest_dir)
    flags = set(args.flags)

    for root, _, files in os.walk(src_dir):
        for file in files:
            rel_path = Path(root).relative_to(src_dir) / file
            src_path = src_dir / rel_path
            dest_path = dest_dir / rel_path
            process_file(src_path, dest_path, flags)

if __name__ == '__main__':
    main()
