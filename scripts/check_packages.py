#!/usr/bin/env python3
"""Verify coordinated archives before the new shared core exists on crates.io.

The only patch points at the *extracted core archive*. Normalized manifests must
retain registry versions, so release consumers will not need a sibling checkout.
A final unpatched publish dry-run is required after publishing the core.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
KIT = ROOT / 'integrations/gpui-kit'


def cargo(*arguments):
    subprocess.run(['cargo', *arguments], cwd=ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native-core', action='store_true', help='Verify the CE archive and all native examples only')
    args = parser.parse_args()

    def package(directory, config=None):
        command = ['cargo', 'metadata', '--no-deps', '--format-version=1',
                   '--manifest-path', str(directory / 'Cargo.toml')]

        if config:
            command += ['--config', config]

        data = json.loads(subprocess.check_output(command, cwd=ROOT, text=True))
        return next(item for item in data['packages'] if Path(item['manifest_path']).parent == directory)

    if (ROOT / 'examples/motion_lab/view.rs').read_bytes() != (KIT / 'examples/motion_lab/view.rs').read_bytes():
        raise ValueError('keep the CE and Kit lab views identical')

    core_version = package(ROOT)['version']
    kit_version = package(KIT)['version']

    cargo('package', '--allow-dirty', '--locked', '--features', 'gpui' if args.native_core else 'text,svg-path')

    core_archive = ROOT / f'target/package/gpui-ce-motion-{core_version}'

    if args.native_core:
        cargo('check', '--locked', '--manifest-path', str(core_archive / 'Cargo.toml'),
              '--target-dir', str(ROOT / 'target'), '--features', 'demo', '--examples')
        print(f'Verified native core {core_version} archive and all CE examples.')
        return

    patch = 'patch.crates-io.gpui-ce-motion.path=' + json.dumps(str(core_archive))

    # Removing the development path in a temporary copy makes the archive patch
    # apply during package verification without changing the repository lockfile.
    with tempfile.TemporaryDirectory(prefix='gpui-motion-kit-package-') as directory:
        staged = Path(directory) / 'kit'
        shutil.copytree(KIT, staged, ignore=shutil.ignore_patterns('target', '.git'))
        source = staged / 'Cargo.toml'
        manifest = source.read_text()
        expected = 'gpui-ce-motion = { path = "../..", '

        if expected not in manifest:
            raise ValueError('unexpected development engine dependency')

        source.write_text(manifest.replace(expected, 'gpui-ce-motion = { ', 1))
        cargo('package', '--manifest-path', str(source), '--target-dir', str(KIT / 'target'),
              '--allow-dirty', '--locked', '--config', patch)

    kit_archive = KIT / f'target/package/gpui-motion-kit-{kit_version}'
    manifest = package(kit_archive, patch)
    dependency = next(item for item in manifest['dependencies'] if item['name'] == 'gpui-ce-motion')

    if dependency.get('path') or not dependency['source'].startswith('registry+') or dependency['req'] != '^' + core_version:
        raise ValueError('Kit archive must depend on the coordinated registry engine version')

    cargo('check', '--locked', '--manifest-path', str(kit_archive / 'Cargo.toml'),
          '--target-dir', str(KIT / 'target'), '--config', patch,
          '--features', 'demo', '--examples')

    print(f'Verified core {core_version} and Kit {kit_version} archives and all Kit examples.')
    print('Unpatched registry verification remains a release step after core publication.')


if __name__ == '__main__':
    main()
