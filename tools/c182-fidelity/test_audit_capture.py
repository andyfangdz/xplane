import copy
import unittest

from audit_capture import audit


def fixture():
    return [
        {"type": "metadata", "schema": 1},
        {"type": "server", "name": "Prepar3D", "major": 5},
        *[dict(type="sample", title="Test aircraft", wall_elapsed_s=i,
               sim_time_s=1000+i, sim_rate=1, paused=0, running=1,
               on_ground=1, weight_lb=3000, oat_c=15, rpm=1000,
               map_inhg=15, fuel_flow_gph=4) for i in range(16)],
        {"type": "end", "samples": 16, "errors": 0, "simulator_quit": False},
    ]


class AuditTests(unittest.TestCase):
    def test_valid_capture_never_proves_fidelity(self):
        result = audit(fixture(), "Test aircraft")
        self.assertTrue(result["capture_usable_for_review"])
        self.assertFalse(result["accepted_baseline"])
        self.assertFalse(result["fidelity_matched"])

    def test_bad_conditions_cannot_be_averaged_away(self):
        for field, value in [("paused", 1), ("paused", -1), ("running", -1),
                             ("sim_rate", 2), ("title", "Different aircraft"),
                             ("rpm", float("nan")), ("fuel_flow_gph", None)]:
            with self.subTest(field=field, value=value):
                records = fixture()
                records[6][field] = value
                self.assertFalse(audit(records, "Test aircraft")["capture_usable_for_review"])

    def test_incomplete_and_truncated_captures_are_rejected(self):
        full = fixture()
        for records in [[], full[:-1], full[1:], full[:2] + full[-1:], full + [full[-1]]]:
            with self.subTest(length=len(records)):
                self.assertFalse(audit(records, "Test aircraft")["capture_usable_for_review"])

    def test_clock_reset_and_duplicate_samples_are_rejected(self):
        for field in ("wall_elapsed_s", "sim_time_s"):
            records = fixture()
            records[8][field] = records[7][field]
            self.assertFalse(audit(records, "Test aircraft")["capture_usable_for_review"])

    def test_count_mismatch_and_sdk_errors_are_rejected(self):
        for change in ({"samples": 99}, {"errors": 1}, {"simulator_quit": True}):
            records = fixture()
            records[-1].update(change)
            self.assertFalse(audit(records, "Test aircraft")["capture_usable_for_review"])
        records = fixture()
        records.insert(2, {"type": "exception", "code": 7})
        self.assertFalse(audit(records, "Test aircraft")["capture_usable_for_review"])

    def test_summarizing_does_not_modify_evidence(self):
        records = fixture()
        original = copy.deepcopy(records)
        audit(records, "Test aircraft")
        self.assertEqual(records, original)


if __name__ == "__main__":
    unittest.main()
