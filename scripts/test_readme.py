"""Covers fenced-example extraction and byte-accurate diagnostic/fix mapping."""

import unittest
from check_readme import apply_fixes, examples, generated_source, mapped_span


class ReadmeTests(unittest.TestCase):
    def test_languages_and_nested_fences(self):
        markdown = '````text\n```rust\nnot code\n```\n````\n\n~~~rust,no_run\nlet value = 1;\n~~~\n'
        blocks = examples(markdown)

        self.assertEqual(len(blocks), 1)
        self.assertEqual(blocks[0].code, 'let value = 1;\n')
        self.assertEqual(blocks[0].markdown_line, 8)

    def test_multiple_examples_have_separate_scopes(self):
        markdown = '```rust\nlet value = 1;\n```\n\n```rust\nlet value = "different type";\n```\n'
        source = generated_source(examples(markdown))

        self.assertIn('mod example_1', source)
        self.assertIn('mod example_2', source)

    def test_utf8_mapping_and_safe_fix_preserve_prose(self):
        markdown = 'Café\n\n```rust\nlet café = 1;\nprintln!("{café}");\n```\n'
        blocks = examples(markdown)
        generated_source(blocks)
        relative_start = len('let café = 1;'.encode())
        span = {
            'byte_start': blocks[0].generated_start + relative_start,
            'byte_end': blocks[0].generated_start + relative_start + 1,
            'suggested_replacement': '\n\n',
            'suggestion_applicability': 'MachineApplicable',
        }
        diagnostic = {'children': [{'spans': [span]}]}

        self.assertIsNotNone(mapped_span(span, blocks))
        self.assertEqual(apply_fixes(markdown, [diagnostic], blocks), markdown.replace('1;\n', '1;\n\n'))

        span['suggested_replacement'] = 'different code'

        with self.assertRaises(ValueError):
            apply_fixes(markdown, [diagnostic], blocks)

    def test_unclosed_or_unverified_examples_fail(self):
        for markdown in ['```rust\nlet value = 1;\n', '```rust,ignore\nlet value = 1;\n```\n', '```rust,compile_fail\ninvalid\n```\n']:
            with self.subTest(markdown=markdown), self.assertRaises(ValueError):
                examples(markdown)


if __name__ == '__main__':
    unittest.main()
