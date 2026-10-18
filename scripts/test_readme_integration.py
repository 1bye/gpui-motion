"""Real Dylint and native dependencies: README failure, safe fix, and compiler errors."""

from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
import json
import subprocess
import tempfile
import unittest

from check_readme import check, consumer_manifest
from check_spacing import check_sources


PROJECT = Path(__file__).resolve().parents[1]


class ReadmeIntegrationTests(unittest.TestCase):
    def test_spacing_fails_then_fixes_without_changing_prose(self):
        markdown = 'Café: keep this text.\n\n```rust\nlet café = 1;\nprintln!("{café}");\n```\n\n```rust\nlet café = "different type";\nprintln!("{café}");\n```\n'

        with tempfile.TemporaryDirectory() as directory, redirect_stdout(StringIO()) as output:
            readme = Path(directory) / 'README.md'
            readme.write_text(markdown)

            self.assertEqual(check(PROJECT, readme), 1)
            self.assertIn(f'{readme}:4:', output.getvalue())
            self.assertEqual(check(PROJECT, readme, fix=True), 0)
            self.assertEqual(check(PROJECT, readme), 0)
            self.assertEqual(readme.read_text(), markdown.replace('1;\n', '1;\n\n').replace('type";\n', 'type";\n\n'))

    def test_source_warnings_fail_the_ci_command(self):
        metadata = json.loads(subprocess.check_output(
            ['cargo', 'metadata', '--no-deps', '--format-version=1', '--manifest-path', str(PROJECT / 'Cargo.toml')],
            text=True,
        ))
        libraries = metadata['metadata']['dylint']['libraries']

        with tempfile.TemporaryDirectory() as directory, redirect_stdout(StringIO()) as output:
            consumer = Path(directory)
            (consumer / 'src').mkdir()
            manifest = consumer_manifest(PROJECT, libraries) + '\n[features]\ndemo = []\nnative-tests = []\n'
            (consumer / 'Cargo.toml').write_text(manifest)
            (consumer / 'Cargo.lock').write_bytes((PROJECT / 'Cargo.lock').read_bytes())
            source = consumer / 'src/lib.rs'
            source.write_text('fn sample() {\n    let value = 1;\n    println!("{value}");\n}\n')
            subprocess.run(['cargo', 'metadata', '--no-deps', '--format-version=1', '--manifest-path', str(consumer / 'Cargo.toml')], capture_output=True, check=True)

            self.assertEqual(check_sources(consumer), 1)
            self.assertIn('insert a blank line', output.getvalue())

            source.write_text(source.read_text().replace('1;\n', '1;\n\n'))

            self.assertEqual(check_sources(consumer), 0)

    def test_broken_code_reports_readme_line_and_fails(self):
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(StringIO()) as output:
            readme = Path(directory) / 'README.md'
            readme.write_text('```rust\nmissing_function();\n```\n')

            self.assertNotEqual(check(PROJECT, readme), 0)
            self.assertIn(f'{readme}:2:', output.getvalue())


if __name__ == '__main__':
    unittest.main()
