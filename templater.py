#!/usr/bin/env python3
import argparse
import os
import re
import difflib
import colorama
from pathlib import Path
from typing import List, Set

CONDITION_RE = re.compile(r"#if\s+(.+)")
ENDIF_RE = re.compile(r"#endif")

def evaluate_condition(condition: str, flags: Set[str], used_flags: Set[str]) -> bool:
    condition = condition.strip()

    if condition.startswith('(and '):
        terms = condition[5:-1].split()
        used_flags.update(terms)
        return all(term in flags for term in terms)
    elif condition.startswith('(or '):
        terms = condition[4:-1].split()
        used_flags.update(terms)
        return any(term in flags for term in terms)
    else:
        used_flags.add(condition)
        return condition in flags

def process_lines(lines: List[str], flags: Set[str], used_flags: Set[str]) -> List[str]:
    output = []
    include_stack = [True]

    for line in lines:
        if match := CONDITION_RE.search(line):
            condition = match.group(1)
            include = evaluate_condition(condition, flags, used_flags)
            include_stack.append(include and include_stack[-1])

            if line.strip() == match.group(0):
                continue  # Entire line is conditional
            else:
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

def process_file(src_path: Path, dest_path: Path, flags: Set[str], used_flags: Set[str]) -> str:
    with src_path.open() as f:
        lines = f.readlines()

    processed = process_lines(lines, flags, used_flags)

    if all(line.strip() == "" for line in processed):
        return "skipped"  # Only whitespace

    dest_path.parent.mkdir(parents=True, exist_ok=True)
    with dest_path.open('w') as f:
        f.write('\n'.join(processed) + '\n')
    return "written"

def scan_all_conditions(src_dir: Path) -> Set[str]:
    seen_flags = set()
    for root, _, files in os.walk(src_dir):
        for file in files:
            with open(Path(root) / file, "r") as f:
                for line in f:
                    if match := CONDITION_RE.search(line):
                        condition = match.group(1).strip()
                        if condition.startswith("(and ") or condition.startswith("(or "):
                            terms = condition[5:-1].split() if condition.startswith("(and ") else condition[4:-1].split()
                            seen_flags.update(terms)
                        else:
                            seen_flags.add(condition)
    return seen_flags

def main():
    parser = argparse.ArgumentParser(description="Template processor with conditional blocks.")
    parser.add_argument('--from', dest='src_dir', default='.', help='Source directory with templates')
    parser.add_argument('--to', dest='dest_dir', default='.', help='Destination directory')
    parser.add_argument('--verbose', action='store_true', help='Print which files were processed or skipped')
    parser.add_argument('flags', nargs='+', help='Flags like `clj devshell` to include conditionals')
    args = parser.parse_args()

    src_dir = Path(args.src_dir)
    dest_dir = Path(args.dest_dir)
    flags = set(args.flags)
    used_flags = set()

    for root, _, files in os.walk(src_dir):
        for file in files:
            rel_path = Path(root).relative_to(src_dir) / file
            src_path = src_dir / rel_path
            dest_path = dest_dir / rel_path
            result = process_file(src_path, dest_path, flags, used_flags)

            if args.verbose:
                if result == "skipped":
                    print(f"Skipped (empty): {rel_path}")
                elif result == "written":
                    print(f"Wrote: {rel_path}")

    unused_flags = flags - used_flags
    if unused_flags:
        colorama.init()

        print("\nUnused flags:")
        all_conditions = scan_all_conditions(src_dir)
        for flag in unused_flags:
            msg = f'  The flag {colorama.Fore.RED}{flag}{colorama.Style.RESET_ALL} isn\'t anywhere in the template files.\n\n'
            msg += f'Available flags are:\n  '
            for used_flag in used_flags:
                msg += f'{used_flag} '
            msg += f'\n\n'
            close = difflib.get_close_matches(flag, all_conditions, n=1)
            if close:
                msg += f'Did you mean {colorama.Fore.GREEN}{close[0]}{colorama.Style.RESET_ALL}?'
            print(msg)

if __name__ == "__main__":
    main()
