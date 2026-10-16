"""Credential boundaries and signing metadata can be checked without Apple credentials or a Mac."""

import base64
from datetime import datetime, timedelta, timezone
import io
import json
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import MagicMock, patch


script = runpy.run_path(str(Path(__file__).with_name("testflight.py")))


class TestFlightTests(unittest.TestCase):
    def profile(self):
        return {
            "TeamIdentifier": ["TEAM123456"], "ApplicationIdentifierPrefix": ["OLDPREFIX1"],
            "Entitlements": {"application-identifier": "OLDPREFIX1.io.github.niiihuel.surco",
                             "com.apple.developer.team-identifier": "TEAM123456", "beta-reports-active": True},
            "ExpirationDate": datetime.now(timezone.utc) + timedelta(days=1), "Platform": ["iOS"],
            "UUID": "d4c803b8-5e55-4027-bf1e-96dd8bb4555c", "DeveloperCertificates": [b"test certificate"],
        }

    def test_missing_credentials_names_only_and_invalid_base64_fail_before_build(self):
        with self.assertRaisesRegex(RuntimeError, "Missing GitHub Secrets: IOS_DISTRIBUTION_P12_BASE64"):
            script["credentials"]({})
        values = {name: base64.b64encode(b"fixture only").decode() for name in script["SECRET_NAMES"]}
        values.update(APPLE_TEAM_ID="TEAM123456", APP_STORE_CONNECT_KEY_ID="KEY1234567",
                      APP_STORE_CONNECT_ISSUER_ID="d4c803b8-5e55-4027-bf1e-96dd8bb4555c")
        self.assertEqual(script["credentials"](values), values)
        values["IOS_DISTRIBUTION_P12_BASE64"] = "invalid!secret"
        with self.assertRaisesRegex(RuntimeError, "IOS_DISTRIBUTION_P12_BASE64 must contain") as error:
            script["credentials"](values)
        self.assertNotIn("invalid!secret", str(error.exception))

    def test_profile_accepts_legacy_prefix_but_rejects_wrong_app_development_and_expiry(self):
        script["validate_profile"](self.profile(), "TEAM123456")
        for kind in ("wrong-app", "development", "expired", "ad-hoc"):
            with self.subTest(kind=kind):
                profile = self.profile()
                if kind == "wrong-app":
                    profile["Entitlements"]["application-identifier"] = "OLDPREFIX1.dnmusic.old"
                elif kind == "development":
                    profile["Entitlements"]["get-task-allow"] = True
                elif kind == "expired":
                    profile["ExpirationDate"] = datetime.now(timezone.utc) - timedelta(seconds=1)
                else:
                    profile["ProvisionedDevices"] = ["test-device"]
                with self.assertRaises(RuntimeError):
                    script["validate_profile"](profile, "TEAM123456")

    def test_es256_der_padding_is_converted_to_exact_jwt_signature(self):
        r = b"\x00\x80" + b"\x00" * 31
        s = b"\x01"
        body = bytes([2, len(r)]) + r + bytes([2, len(s)]) + s
        der = bytes([0x30, len(body)]) + body
        self.assertEqual(script["jwt_signature"](der), r[1:] + s.rjust(32, b"\x00"))
        with self.assertRaises(RuntimeError):
            script["jwt_signature"](der + b"unexpected")

    def test_app_record_lookup_fails_when_surco_has_not_been_created(self):
        values = {"APP_STORE_CONNECT_KEY_ID": "KEY1234567", "APP_STORE_CONNECT_ISSUER_ID": "issuer"}
        der = b"\x30\x06\x02\x01\x01\x02\x01\x02"
        opener = MagicMock()
        opener.open.return_value.__enter__.return_value = io.BytesIO(b'{"data": []}')
        function = script["app_record"]
        with patch.dict(function.__globals__, {"execute": lambda *args, **kwargs: der,
                                               "build_opener": lambda *args: opener}):
            with self.assertRaisesRegex(RuntimeError, "Create the NEW Surco app record"):
                function(values, Path("unused-fixture.p8"), {})
        request = opener.open.call_args.args[0]
        self.assertIn("filter%5BbundleId%5D=io.github.niiihuel.surco", request.full_url)
        self.assertTrue(request.headers["Authorization"].startswith("Bearer "))

    def test_processed_build_is_added_to_every_internal_group(self):
        calls = []
        replies = iter([{"data": []}, {"data": [{"id": "B1", "attributes": {"processingState": "PROCESSING"}}]},
                        {"data": [{"id": "B1", "attributes": {"processingState": "VALID"}}]},
                        {"data": [{"id": "G1", "attributes": {"name": "Surco interno"}}]}, None])
        function = script["deliver"]
        with patch.dict(function.__globals__, {"asc": lambda *args: calls.append(args[3:]) or next(replies)}), \
                patch("sys.stdout", io.StringIO()):
            function({}, None, {}, "APP", "0.6.0", "8.1", wait=lambda seconds: None)
        self.assertIn("filter%5Bversion%5D=8.1", calls[0][0])
        self.assertIn("filter%5BisInternalGroup%5D=true", calls[3][0])
        self.assertEqual(calls[4], ("/v1/betaGroups/G1/relationships/builds", {"data": [{"type": "builds", "id": "B1"}]}))
        replies = iter([{"data": [{"id": "B1", "attributes": {"processingState": "INVALID"}}]}])
        with patch.dict(function.__globals__, {"asc": lambda *args: next(replies)}), patch("sys.stdout", io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, "INVALID"):
                function({}, None, {}, "APP", "0.6.0", "8.1", wait=lambda seconds: None)

    def test_secret_output_is_redacted_before_console_or_artifact_logs(self):
        function = script["execute"]
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "build.log"
            completed = subprocess.CompletedProcess([], 0, stdout=b"output private-value end")
            console = io.StringIO()
            with patch.dict(function.__globals__, {"REDACTIONS": ["private-value"]}), \
                    patch.object(subprocess, "run", return_value=completed), patch("sys.stdout", console):
                function("Build", ["unused"], {}, log=log)
            self.assertNotIn("private-value", console.getvalue())
            self.assertEqual(log.read_text(), "output [REDACTED] end")


if __name__ == "__main__":
    unittest.main()
