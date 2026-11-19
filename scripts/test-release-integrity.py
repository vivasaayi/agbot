import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

SCRIPT = Path(__file__).with_name('release-guard.py')
spec = importlib.util.spec_from_file_location('release_guard', SCRIPT)
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class ReleaseGuardTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.state = Path(self.tmp.name)
        (self.state / 'environment.env').write_text('APP_IMAGE=agbot:commit-a\n')
        self.snapshot = ('commit-a', 'commit-a', False)
        self.image = {'id': 'sha256:good', 'revision': 'commit-a', 'platform': 'linux/arm64'}

    def record(self):
        with mock.patch.object(guard, 'snapshot', return_value=self.snapshot), \
             mock.patch.object(guard, 'inspect_image', return_value=self.image):
            guard.record_build(Path('/repo'), self.state)

    def test_mac_integration_preserves_legacy_project_but_vps_staging_is_separate(self):
        config = json.loads((SCRIPT.parent.parent / 'deploy/product.json').read_text())
        self.assertEqual(config['integration_project'], 'agbot-staging')
        self.assertNotEqual(config['staging_project'], config['integration_project'])
        self.assertNotEqual(config['staging_project'], 'agbot-production')

    def test_default_integration_deploy_requires_a_matching_successful_build(self):
        with mock.patch.object(guard, 'main_revision', return_value='commit-a'), \
             mock.patch.object(guard, 'inspect_image', return_value=self.image):
            with self.assertRaises(ValueError):
                guard.verify_deployment(Path('/repo'), self.state)
        self.record()
        with mock.patch.object(guard, 'main_revision', return_value='commit-a'), \
             mock.patch.object(guard, 'inspect_image', return_value=self.image):
            self.assertEqual(guard.verify_deployment(Path('/repo'), self.state), 'sha256:good')
        with mock.patch.object(guard, 'main_revision', return_value='commit-b'), \
             mock.patch.object(guard, 'inspect_image', return_value=self.image):
            with self.assertRaises(ValueError):
                guard.verify_deployment(Path('/repo'), self.state)
        with mock.patch.object(guard, 'main_revision', return_value='commit-a'), \
             mock.patch.object(guard, 'inspect_image', return_value={**self.image, 'id': 'sha256:replaced'}):
            with self.assertRaises(ValueError):
                guard.verify_deployment(Path('/repo'), self.state)
        for change in ({'revision': 'wrong'}, {'platform': 'linux/amd64'}):
            with self.subTest(change=change), \
                 mock.patch.object(guard, 'main_revision', return_value='commit-a'), \
                 mock.patch.object(guard, 'inspect_image', return_value={**self.image, **change}):
                with self.assertRaises(ValueError):
                    guard.verify_deployment(Path('/repo'), self.state)

    def test_build_is_not_recorded_if_source_or_image_revision_changes(self):
        with mock.patch.object(guard, 'snapshot', side_effect=[self.snapshot, ('commit-b', 'commit-b', False)]), \
             mock.patch.object(guard, 'inspect_image', return_value=self.image):
            with self.assertRaises(ValueError):
                guard.record_build(Path('/repo'), self.state)
        self.assertFalse((self.state / 'build-receipt.json').exists())
        with mock.patch.object(guard, 'snapshot', return_value=('commit-a', 'commit-a', True)), \
             mock.patch.object(guard, 'inspect_image', return_value=self.image):
            with self.assertRaises(ValueError):
                guard.record_build(Path('/repo'), self.state)
        self.assertFalse((self.state / 'build-receipt.json').exists())
        with mock.patch.object(guard, 'snapshot', return_value=self.snapshot), \
             mock.patch.object(guard, 'inspect_image', return_value={**self.image, 'revision': 'wrong'}):
            with self.assertRaises(ValueError):
                guard.record_build(Path('/repo'), self.state)
        self.assertFalse((self.state / 'build-receipt.json').exists())

    def test_receipt_is_private_and_contains_no_credentials(self):
        (self.state / 'environment.env').write_text('APP_IMAGE=agbot:commit-a\nPOSTGRES_PASSWORD=private\n')
        self.record()
        receipt = self.state / 'build-receipt.json'
        self.assertEqual(receipt.stat().st_mode & 0o777, 0o600)
        self.assertNotIn('private', receipt.read_text())
        self.assertEqual(json.loads(receipt.read_text())['image_id'], 'sha256:good')

    def test_production_requires_an_immutable_image_reference(self):
        digest = 'a' * 64
        for image in ('sha256:' + digest, 'registry.example/agbot@sha256:' + digest):
            with self.subTest(image=image):
                self.assertEqual(guard.validate_production_image(image), image)
        for image in ('agbot:latest', 'agbot:commit-tag', 'sha256:short',
                      'registry.example/agbot@sha256:' + 'g' * 64, ''):
            with self.subTest(image=image), self.assertRaises(ValueError):
                guard.validate_production_image(image)

    def test_vps_staging_rejects_tags_and_never_uses_legacy_mac_state(self):
        org = self.state / 'org'
        (org / 'bin').mkdir(parents=True)
        marker = self.state / 'invoked'
        (org / 'bin/product-env.py').write_text(
            'import os, pathlib\n'
            'pathlib.Path(os.environ["TEST_MARKER"]).write_text("called")\n'
        )
        legacy = self.state / 'agbot-staging'
        legacy.mkdir()
        (legacy / 'environment.env').write_text('APP_IMAGE=sha256:' + 'a' * 64 + '\n')
        env = {**os.environ, 'ORG_DEV_ROOT': str(org), 'WORK_ENV_STATE_ROOT': str(self.state), 'TEST_MARKER': str(marker)}
        script = str(Path(__file__).with_name('env'))
        for args in (['staging', 'deploy'], ['staging', 'deploy', '--image', 'agbot:latest']):
            result = subprocess.run(['bash', script, *args], env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(marker.exists())
        result = subprocess.run(['bash', script, 'staging', 'deploy', '--image', 'sha256:' + 'b' * 64], env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(marker.exists())

    def test_launcher_blocks_default_deploy_and_does_not_record_failed_build(self):
        org = self.state / 'org'
        (org / 'bin').mkdir(parents=True)
        marker = self.state / 'invoked'
        (org / 'bin/product-env.py').write_text(
            'import os, pathlib, sys\n'
            'pathlib.Path(os.environ["TEST_MARKER"]).write_text("called")\n'
            'sys.exit(7 if "build" in sys.argv else 0)\n'
        )
        env = {**os.environ, 'ORG_DEV_ROOT': str(org), 'WORK_ENV_STATE_ROOT': str(self.state), 'TEST_MARKER': str(marker)}
        script = str(Path(__file__).with_name('env'))
        denied = subprocess.run(['bash', script, 'integration', 'deploy'], env=env, capture_output=True, text=True)
        self.assertNotEqual(denied.returncode, 0)
        self.assertFalse(marker.exists())
        explicit = subprocess.run(['bash', script, 'integration', 'deploy', '--image', 'example:explicit'], env=env, capture_output=True, text=True)
        self.assertEqual(explicit.returncode, 0, explicit.stderr)
        self.assertTrue(marker.exists())
        marker.unlink()
        old_receipt = self.state / 'agbot-staging/build-receipt.json'
        old_receipt.parent.mkdir(parents=True, exist_ok=True)
        old_receipt.write_text('{"image_id":"sha256:old"}')
        failed = subprocess.run(['bash', script, 'integration', 'build'], env=env, capture_output=True, text=True)
        self.assertEqual(failed.returncode, 7)
        self.assertTrue(marker.exists())
        self.assertFalse(old_receipt.exists())

    def test_production_launcher_rejects_mutable_tags_before_deployment(self):
        org = self.state / 'org'
        (org / 'bin').mkdir(parents=True)
        marker = self.state / 'invoked'
        (org / 'bin/product-env.py').write_text(
            'import json, os, pathlib, sys\n'
            'pathlib.Path(os.environ["TEST_MARKER"]).write_text(json.dumps(sys.argv))\n'
        )
        env = {**os.environ, 'ORG_DEV_ROOT': str(org), 'WORK_ENV_STATE_ROOT': str(self.state), 'TEST_MARKER': str(marker)}
        script = str(Path(__file__).with_name('env'))
        denied = subprocess.run(['bash', script, 'production', 'deploy', '--image', 'agbot:latest'], env=env, capture_output=True, text=True)
        self.assertNotEqual(denied.returncode, 0)
        self.assertFalse(marker.exists())
        reordered = subprocess.run(['bash', script, '--image', 'agbot:latest', 'production', 'deploy'], env=env, capture_output=True, text=True)
        self.assertNotEqual(reordered.returncode, 0)
        self.assertFalse(marker.exists())
        missing = subprocess.run(['bash', script, 'production', 'up'], env=env, capture_output=True, text=True)
        self.assertNotEqual(missing.returncode, 0)
        self.assertFalse(marker.exists())
        image = 'sha256:' + 'b' * 64
        allowed = subprocess.run(['bash', script, 'production', 'deploy', '--image', image], env=env, capture_output=True, text=True)
        self.assertEqual(allowed.returncode, 0, allowed.stderr)
        self.assertEqual(json.loads(marker.read_text())[-2:], ['--image', image])
        marker.unlink()
        allowed_equals = subprocess.run(['bash', script, 'production', 'up', '--image=' + image], env=env, capture_output=True, text=True)
        self.assertEqual(allowed_equals.returncode, 0, allowed_equals.stderr)
        self.assertEqual(json.loads(marker.read_text())[-2:], ['--image', image])
        marker.unlink()
        state = self.state / 'agbot-production'
        state.mkdir()
        (state / 'environment.env').write_text('APP_IMAGE=' + image + '\n')
        resumed = subprocess.run(['bash', script, 'production', 'up'], env=env, capture_output=True, text=True)
        self.assertEqual(resumed.returncode, 0, resumed.stderr)
        self.assertEqual(json.loads(marker.read_text())[-2:], ['--image', image])
        marker.unlink()
        (state / 'environment.env').write_text('APP_IMAGE=agbot:latest\n')
        stale = subprocess.run(['bash', script, 'production', 'up'], env=env, capture_output=True, text=True)
        self.assertNotEqual(stale.returncode, 0)
        self.assertFalse(marker.exists())

    def test_launcher_records_successful_build_and_pins_deploy(self):
        org = self.state / 'org'
        (org / 'bin').mkdir(parents=True)
        fake_bin = self.state / 'bin'
        fake_bin.mkdir()
        fake_git = fake_bin / 'git'
        fake_git.write_text('#!/bin/sh\ncase "$*" in *status*) exit 0;; *) echo commit-a;; esac\n')
        fake_git.chmod(0o755)
        fake_docker = fake_bin / 'docker'
        fake_docker.write_text('#!/bin/sh\nprintf \'%s\\n\' \'[{"Id":"sha256:good","Os":"linux","Architecture":"arm64","Config":{"Labels":{"org.opencontainers.image.revision":"commit-a"}}}]\'\n')
        fake_docker.chmod(0o755)
        marker = self.state / 'invoked'
        (org / 'bin/product-env.py').write_text(
            'import json, os, pathlib, sys\n'
            'state=pathlib.Path(os.environ["WORK_ENV_STATE_ROOT"])/"agbot-staging"\n'
            'state.mkdir(parents=True, exist_ok=True)\n'
            '(state/"environment.env").write_text("APP_IMAGE=agbot:commit-a\\n")\n'
            'pathlib.Path(os.environ["TEST_MARKER"]).write_text(json.dumps(sys.argv))\n'
        )
        env = {**os.environ, 'PATH': str(fake_bin) + os.pathsep + os.environ['PATH'],
               'ORG_DEV_ROOT': str(org), 'WORK_ENV_STATE_ROOT': str(self.state), 'TEST_MARKER': str(marker)}
        script = str(Path(__file__).with_name('env'))
        build = subprocess.run(['bash', script, 'integration', 'build'], env=env, capture_output=True, text=True)
        self.assertEqual(build.returncode, 0, build.stderr)
        receipt = json.loads((self.state / 'agbot-staging/build-receipt.json').read_text())
        self.assertEqual(receipt['image_id'], 'sha256:good')
        deploy = subprocess.run(['bash', script, 'integration', 'deploy'], env=env, capture_output=True, text=True)
        self.assertEqual(deploy.returncode, 0, deploy.stderr)
        self.assertEqual(json.loads(marker.read_text())[-2:], ['--image', 'sha256:good'])

    def test_real_git_detects_build_changes_and_default_branch_advances(self):
        repo = self.state / 'repo'
        (repo / 'scripts').mkdir(parents=True)
        (repo / 'deploy').mkdir()
        shutil.copyfile(SCRIPT, repo / 'scripts/release-guard.py')
        shutil.copyfile(SCRIPT.with_name('env'), repo / 'scripts/env')
        (repo / 'deploy/product.json').write_text('{"default_branch":"main","integration_project":"agbot-staging"}')
        (repo / 'source.txt').write_text('original')

        def git(*args):
            return subprocess.run(['git', '-C', str(repo), *args], check=True, capture_output=True, text=True)

        git('init', '-q', '-b', 'main')
        git('add', 'scripts', 'deploy', 'source.txt')
        git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-qm', 'fixture')
        org = self.state / 'org/bin'
        org.mkdir(parents=True)
        marker = self.state / 'invoked'
        (org / 'product-env.py').write_text(
            'import json, os, pathlib, subprocess, sys\n'
            'root=pathlib.Path(sys.argv[sys.argv.index("--repo")+1])\n'
            'state=pathlib.Path(os.environ["WORK_ENV_STATE_ROOT"])/"agbot-staging"\n'
            'state.mkdir(parents=True, exist_ok=True)\n'
            '(state/"environment.env").write_text("APP_IMAGE=agbot:fixture\\n")\n'
            'if "build" in sys.argv and os.environ.get("TEST_CHANGE") == "dirty":\n'
            ' (root/"source.txt").write_text("modified during build")\n'
            'if "build" in sys.argv and os.environ.get("TEST_CHANGE") == "advance":\n'
            ' subprocess.run(["git", "-C", str(root), "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-q", "--allow-empty", "-m", "advance"], check=True)\n'
            'pathlib.Path(os.environ["TEST_MARKER"]).write_text(json.dumps(sys.argv))\n'
        )
        fake_bin = self.state / 'bin'
        fake_bin.mkdir()
        docker = fake_bin / 'docker'
        docker.write_text(
            '#!/usr/bin/env python3\nimport json, os\n'
            'print(json.dumps([{"Id":os.environ.get("TEST_IMAGE_ID", "sha256:original"), "Os":"linux", "Architecture":"arm64", "Config":{"Labels":{"org.opencontainers.image.revision":os.environ["AGBOT_SOURCE_COMMIT"]}}}]))\n'
        )
        docker.chmod(0o755)
        env = {**os.environ, 'PATH': str(fake_bin) + os.pathsep + os.environ['PATH'],
               'ORG_DEV_ROOT': str(self.state / 'org'), 'WORK_ENV_STATE_ROOT': str(self.state / 'state'), 'TEST_MARKER': str(marker)}
        script = str(repo / 'scripts/env')
        build = subprocess.run(['bash', script, 'integration', 'build'], env=env, capture_output=True, text=True)
        self.assertEqual(build.returncode, 0, build.stderr)
        deploy = subprocess.run(['bash', script, 'integration', 'deploy'], env=env, capture_output=True, text=True)
        self.assertEqual(deploy.returncode, 0, deploy.stderr)
        self.assertEqual(json.loads(marker.read_text())[-2:], ['--image', 'sha256:original'])
        marker.unlink()
        replaced = subprocess.run(['bash', script, 'integration', 'deploy'], env={**env, 'TEST_IMAGE_ID': 'sha256:replaced'}, capture_output=True, text=True)
        self.assertNotEqual(replaced.returncode, 0)
        self.assertFalse(marker.exists())
        git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-q', '--allow-empty', '-m', 'new main')
        stale = subprocess.run(['bash', script, 'integration', 'deploy'], env=env, capture_output=True, text=True)
        self.assertNotEqual(stale.returncode, 0)
        self.assertFalse(marker.exists())
        receipt = self.state / 'state/agbot-staging/build-receipt.json'
        receipt.unlink()
        for scenario in ('dirty', 'advance'):
            with self.subTest(scenario=scenario):
                if scenario == 'advance':
                    git('checkout', '--', 'source.txt')
                changed = subprocess.run(['bash', script, 'integration', 'build'], env={**env, 'TEST_CHANGE': scenario}, capture_output=True, text=True)
                self.assertNotEqual(changed.returncode, 0)
                self.assertFalse(receipt.exists())


if __name__ == '__main__':
    unittest.main()
