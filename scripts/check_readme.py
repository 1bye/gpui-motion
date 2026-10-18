#!/usr/bin/env python3
"""Compile and Dylint README Rust examples using the project's pinned lint library."""

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


SPACING_CODES = {"statement_padding", "item_padding", "multiple_empty_lines", "padded_blocks"}


@dataclass
class Example:
    code: str
    markdown_start: int
    markdown_line: int
    generated_start: int = 0
    generated_line: int = 0


def examples(markdown):
    """Read fenced examples, preserving byte offsets for diagnostics and fixes."""
    result = []
    fence = None
    rust = False
    code = []
    offset = 0
    start = 0
    first_line = 0

    for number, line in enumerate(markdown.splitlines(keepends=True), 1):
        opening = re.match(r"^ {0,3}(`{3,}|~{3,})([^\r\n]*)", line)

        if fence is None and opening:
            fence = opening[1]
            flags = opening[2].strip().split(',')
            rust = flags[0].strip() in {"rust", "rs"}

            if rust and any(flag.strip().startswith(("ignore", "compile_fail")) for flag in flags):
                raise ValueError(f"README:{number}: ignored/compile-fail examples need separate verification")

            code = []
            start = offset + len(line.encode())
            first_line = number + 1
        elif fence is not None and re.fullmatch(r" {0,3}" + re.escape(fence[0]) + "{" + str(len(fence)) + r",}[ \t]*[\r\n]*", line):
            if rust:
                result.append(Example(''.join(code), start, first_line))

            fence = None
        elif fence is not None and rust:
            code.append(line)

        offset += len(line.encode())

    if fence is not None:
        raise ValueError("README has an unclosed fenced block")

    return result


def generated_source(blocks):
    source = '#![allow(dead_code, unused_imports, unused_variables)]\n'

    for index, example in enumerate(blocks, 1):
        source += f'\nmod example_{index} {{\nfn main() {{\n'
        example.generated_start = len(source.encode())
        example.generated_line = source.count('\n') + 1
        source += example.code
        source += '}\n}\n'

    return source


def mapped_span(span, blocks):
    for example in blocks:
        start = example.generated_start

        if start <= span['byte_start'] <= span['byte_end'] <= start + len(example.code.encode()):
            return example, example.markdown_start + span['byte_start'] - start, example.markdown_start + span['byte_end'] - start

    return None


def apply_fixes(markdown, diagnostics, blocks):
    changes = {}

    for diagnostic in diagnostics:
        for child in diagnostic.get('children', []):
            for span in child.get('spans', []):
                replacement = span.get('suggested_replacement')

                if replacement is None or span.get('suggestion_applicability') != 'MachineApplicable':
                    continue

                mapped = mapped_span(span, blocks)

                if mapped is None:
                    raise ValueError('lint suggested changing the generated example wrapper')

                _, start, end = mapped
                original = markdown.encode()[start:end].decode()

                if re.sub(r'\s', '', original) != re.sub(r'\s', '', replacement):
                    raise ValueError('spacing fix would change non-whitespace content')

                changes[start, end] = replacement.encode()

    output = markdown.encode()
    boundary = len(output) + 1

    for (start, end), replacement in sorted(changes.items(), reverse=True):
        if end > boundary:
            continue  # Overlapping block/empty-line fixes are handled on the next pass.

        output = output[:start] + replacement + output[end:]
        boundary = start

    return output.decode()


def consumer_manifest(project, libraries):
    manifest = '''[package]
name = "gpui-motion-readme"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
gpui-ce-motion = { path = PROJECT, features = ["gpui"] }
gpui = { package = "gpui-ce", version = "=0.2.2", default-features = false }

[workspace.metadata.dylint]
libraries = LIBRARIES
'''
    entries = []

    for library in libraries:
        library = dict(library)

        if 'path' in library:
            library['path'] = str((project / library['path']).resolve())

        entries.append('{ ' + ', '.join(key + ' = ' + json.dumps(value) for key, value in library.items()) + ' }')

    return manifest.replace('PROJECT', json.dumps(str(project))).replace('LIBRARIES', '[\n' + ',\n'.join(entries) + '\n]')


def check(project, readme, fix=False, library=None):
    metadata = subprocess.run(
        ['cargo', 'metadata', '--no-deps', '--format-version=1', '--locked', '--manifest-path', str(project / 'Cargo.toml')],
        capture_output=True, text=True, check=True,
    )
    libraries = json.loads(metadata.stdout)['metadata']['dylint']['libraries']
    environment = dict(os.environ)
    environment.setdefault('CARGO_TARGET_DIR', str(project / 'target'))
    environment['CARGO_INCREMENTAL'] = '0'

    with tempfile.TemporaryDirectory(prefix='gpui-motion-readme-') as directory:
        consumer = Path(directory)
        (consumer / 'src').mkdir()
        (consumer / 'Cargo.toml').write_text(consumer_manifest(project, libraries))
        (consumer / 'Cargo.lock').write_bytes((project / 'Cargo.lock').read_bytes())

        for attempt in range(6 if fix else 1):
            markdown = readme.read_bytes().decode()
            blocks = examples(markdown)

            if not blocks:
                raise ValueError('README contains no Rust examples')

            (consumer / 'src/lib.rs').write_text(generated_source(blocks))
            selection = ['--path', str(library)] if library else ['--all']
            command = ['cargo', 'dylint', *selection, '--no-deps', '--manifest-path', str(consumer / 'Cargo.toml'), '--', '--message-format=json']
            result = subprocess.run(command, cwd=consumer, env=environment, capture_output=True, text=True)
            diagnostics = []

            for line in result.stdout.splitlines():
                if not line.startswith('{'):
                    continue

                event = json.loads(line)

                if event.get('reason') == 'compiler-message':
                    diagnostic = event['message']

                    if (diagnostic.get('code') or {}).get('code') in SPACING_CODES:
                        diagnostics.append(diagnostic)
                    elif diagnostic['level'] == 'error':
                        report(diagnostic, blocks, readme)

            if result.returncode:
                print(result.stderr)
                return result.returncode

            if not diagnostics:
                print(f'README: {len(blocks)} examples compile and pass Dylint spacing.')
                return 0

            if fix:
                updated = apply_fixes(markdown, diagnostics, blocks)

                if updated == markdown:
                    raise ValueError('spacing diagnostics could not be fixed safely')

                readme.write_bytes(updated.encode())
                continue

            for diagnostic in diagnostics:
                report(diagnostic, blocks, readme)

            return 1

    raise ValueError('spacing fixes did not converge after six passes')


def report(diagnostic, blocks, readme):
    primary = next((span for span in diagnostic['spans'] if span['is_primary']), None)
    mapped = mapped_span(primary, blocks) if primary else None

    if mapped:
        example, _, _ = mapped
        line = example.markdown_line + primary['line_start'] - example.generated_line
        code = (diagnostic.get('code') or {}).get('code', diagnostic['level'])
        print(f'{readme}:{line}: {diagnostic["message"]} [{code}]')
    else:
        print(diagnostic.get('rendered') or diagnostic['message'])


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--readme', type=Path)
    parser.add_argument('--fix', action='store_true', help='Apply only machine-applicable whitespace fixes')
    parser.add_argument('--library', type=Path, help='Use a local lint checkout while developing the shared policy')
    args = parser.parse_args()

    try:
        raise SystemExit(check(args.project.resolve(), args.readme or args.project / 'README.md', args.fix, args.library))
    except (ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'{error}\n')
