#!/usr/bin/env python3
"""Check all native Rust targets and README examples with the pinned spacing policy."""

import argparse
import json
import os
from pathlib import Path
import subprocess

from check_readme import SPACING_CODES, check as check_readme


def check_sources(project, fix=False, library=None):
    selection = ['--path', str(library)] if library else ['--all']
    command = ['cargo', 'dylint', *selection, '--no-deps']

    if fix:
        command += ['--fix']

    command += ['--', '--locked', '--all-targets', '--features', 'demo,native-tests', '--message-format=json']

    if fix:
        command += ['--allow-dirty', '--allow-staged']

    environment = dict(os.environ)
    environment['CARGO_TERM_COLOR'] = 'never'
    result = subprocess.run(command, cwd=project, env=environment, capture_output=True, text=True)
    diagnostics = []

    for line in result.stdout.splitlines():
        if line.startswith('{'):
            event = json.loads(line)

            if event.get('reason') == 'compiler-message':
                message = event['message']

                if (message.get('code') or {}).get('code') in SPACING_CODES or message['level'] == 'error':
                    diagnostics.append(message)

    if result.returncode:
        print(result.stderr)

    for diagnostic in diagnostics:
        print(diagnostic.get('rendered') or diagnostic['message'])

    if fix and result.returncode == 0:
        return check_sources(project, library=library)

    if result.returncode or diagnostics:
        return result.returncode or 1

    print('Rust: all native targets pass Dylint spacing.')
    return 0


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fix', action='store_true', help='Apply machine-applicable whitespace fixes')
    parser.add_argument('--library', type=Path, help='Use a local lint checkout while developing the shared policy')
    parser.add_argument('--project', type=Path, default=Path(__file__).resolve().parents[1], help='Cargo package to check')
    parser.add_argument('--sources-only', action='store_true', help='Check Rust targets without extracting README examples')
    args = parser.parse_args()
    project = args.project.resolve()

    try:
        source_result = check_sources(project, args.fix, args.library)
        readme_result = 0 if args.sources_only else check_readme(project, project / 'README.md', args.fix, args.library)
        raise SystemExit(source_result or readme_result)
    except (ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'{error}\n')
