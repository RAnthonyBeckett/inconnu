"""A failed new process must never inherit evidence from an older successful run."""
import tempfile
import unittest
from pathlib import Path
from native_smoke import current_run


class CurrentRunEvidence(unittest.TestCase):
    def test_incomplete_current_run_is_selected_over_old_success(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            old = output / 'ui-smoke-old'
            old.mkdir()
            (old / 'ui-smoke.json').write_text('{"ok": true}')
            before = set(output.glob('ui-smoke-*'))
            new = output / 'ui-smoke-new'
            new.mkdir()
            selected = current_run(output, before)
            self.assertEqual(selected, new)
            self.assertFalse((selected / 'ui-smoke.json').exists())

    def test_no_new_run_rejects_old_success(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            old = output / 'ui-smoke-old'
            old.mkdir()
            (old / 'ui-smoke.json').write_text('{"ok": true}')
            with self.assertRaises(RuntimeError):
                current_run(output, {old})

    def test_two_current_runs_are_ambiguous(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            for name in ['ui-smoke-one', 'ui-smoke-two']:
                (output / name).mkdir()
            with self.assertRaises(RuntimeError):
                current_run(output, set())


if __name__ == '__main__':
    unittest.main()
