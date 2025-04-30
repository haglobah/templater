#!/usr/bin/env python3
import unittest
from pathlib import Path
import tempfile
import shutil
from templater import process_file, scan_all_conditions
import difflib

class TestTemplater(unittest.TestCase):
    def setUp(self):
        self.tmpdir = tempfile.mkdtemp()
        self.src_dir = Path(self.tmpdir) / "src"
        self.out_dir = Path(self.tmpdir) / "out"
        self.src_dir.mkdir()
        self.flags = set()
        self.used_flags = set()

    def tearDown(self):
        shutil.rmtree(self.tmpdir)

    def write_template(self, filename, content):
        path = self.src_dir / filename
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        return path

    def read_output(self, filename):
        return (self.out_dir / filename).read_text()

    def test_basic_inclusion(self):
        self.write_template("basic.txt", "foo\n#if bar\nbar\n#endif\nbaz\n")
        self.flags = {"bar"}
        process_file(self.src_dir / "basic.txt", self.out_dir / "basic.txt", self.flags, self.used_flags)
        self.assertEqual(self.read_output("basic.txt"), "foo\nbar\nbaz\n")

    def test_basic_exclusion(self):
        self.write_template("exclude.txt", "foo\n#if bar\nbar\n#endif\nbaz\n")
        self.flags = {"notbar"}
        process_file(self.src_dir / "exclude.txt", self.out_dir / "exclude.txt", self.flags, self.used_flags)
        self.assertEqual(self.read_output("exclude.txt"), "foo\nbaz\n")

    def test_and_condition(self):
        self.write_template("and.txt", "#if (and foo bar)\nhello\n#endif\n")
        self.flags = {"foo", "bar"}
        process_file(self.src_dir / "and.txt", self.out_dir / "and.txt", self.flags, self.used_flags)
        self.assertIn("hello", self.read_output("and.txt"))

    def test_or_condition(self):
        self.write_template("or.txt", "#if (or foo bar)\nhello\n#endif\n")
        self.flags = {"foo"}
        process_file(self.src_dir / "or.txt", self.out_dir / "or.txt", self.flags, self.used_flags)
        self.assertIn("hello", self.read_output("or.txt"))

    def test_nested_conditions(self):
        content = "#if foo\nfoo\n  #if bar\nfoobar\n  #endif\n#endif\n"
        self.write_template("nested.txt", content)
        self.flags = {"foo", "bar"}
        process_file(self.src_dir / "nested.txt", self.out_dir / "nested.txt", self.flags, self.used_flags)
        out = self.read_output("nested.txt")
        self.assertIn("foobar", out)

    def test_skips_empty_file(self):
        self.write_template("empty.txt", "#if bar\n#endif\n")
        self.flags = set()
        result = process_file(self.src_dir / "empty.txt", self.out_dir / "empty.txt", self.flags, self.used_flags)
        self.assertEqual(result, "skipped")
        self.assertFalse((self.out_dir / "empty.txt").exists())

    def test_used_flags_tracking(self):
        self.write_template("track.txt", "#if foo\nx\n#endif\n")
        self.flags = {"foo", "bar"}
        used = set()
        process_file(self.src_dir / "track.txt", self.out_dir / "track.txt", self.flags, used)
        self.assertIn("foo", used)
        self.assertNotIn("bar", used)

    def test_close_match_suggestion(self):
        self.write_template("flags.txt", "#if devshell\nx\n#endif\n")
        all_flags = scan_all_conditions(self.src_dir)
        close = "devsel"
        suggestion = difflib.get_close_matches(close, all_flags, n=1)
        self.assertIn("devshell", suggestion)

if __name__ == '__main__':
    unittest.main()
