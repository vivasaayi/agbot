#!/usr/bin/env python3
"""Record a verified integration build and reject stale default deployments."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def main_revision(root):
    branch = json.loads((root / 'deploy/product.json').read_text())['default_branch']
    return git(root, 'rev-parse', '--verify', f'refs/heads/{branch}^{{commit}}')


def snapshot(root):
    return (git(root, 'rev-parse', 'HEAD'), main_revision(root), bool(git(root, 'status', '--porcelain')))


def selected_image(state):
    values = {}
    for line in (state / 'environment.env').read_text().splitlines():
        if line and not line.startswith('#'):
            key, value = line.split('=', 1)
            values[key] = value
    image = values.get('APP_IMAGE', '')
    if not image or any(c.isspace() for c in image):
        raise ValueError('environment has no valid selected image')
    return image


def inspect_image(image):
    output = subprocess.check_output(['docker', 'image', 'inspect', image], text=True)
    details = json.loads(output)[0]
    return {
        'id': details['Id'],
        'revision': (details.get('Config', {}).get('Labels') or {}).get('org.opencontainers.image.revision'),
        'platform': details['Os'] + '/' + details['Architecture'],
    }


def record_build(root, state):
    before = snapshot(root)
    if before[2] or before[0] != before[1]:
        raise ValueError('integration build source is no longer a clean default-branch revision')
    image = inspect_image(selected_image(state))
    if image['revision'] != before[0]:
        raise ValueError('built image revision does not match the default branch')
    if snapshot(root) != before:
        raise ValueError('source changed while recording the integration build')
    receipt = {'source_revision': before[0], 'image_id': image['id'], 'platform': image['platform']}
    state.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix='.build-receipt-', dir=state)
    try:
        with os.fdopen(descriptor, 'w') as output:
            json.dump(receipt, output, indent=2)
            output.write('\n')
        os.replace(temporary, state / 'build-receipt.json')
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def verify_deployment(root, state):
    receipt_path = state / 'build-receipt.json'
    if not receipt_path.exists():
        raise ValueError('no verified integration build; run bash scripts/build integration')
    receipt = json.loads(receipt_path.read_text())
    revision = main_revision(root)
    image = inspect_image(selected_image(state))
    if (receipt.get('source_revision') != revision or image['revision'] != revision
            or receipt.get('image_id') != image['id'] or receipt.get('platform') != image['platform']
            or main_revision(root) != revision):
        raise ValueError('selected integration image is stale or differs from the verified build; rebuild integration')
    return image['id']


def validate_production_image(image):
    digest = r'sha256:[0-9a-f]{64}'
    if not (re.fullmatch(digest, image) or re.fullmatch(r'[^@\s]+@' + digest, image)):
        raise ValueError('VPS staging and production require a full image ID or registry digest, not a mutable tag')
    return image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('invalidate', 'record', 'verify', 'staging', 'production'))
    parser.add_argument('root', type=Path)
    parser.add_argument('--image')
    args = parser.parse_args()
    state_root = Path(os.environ.get('WORK_ENV_STATE_ROOT', str(Path.home() / '.local/share/work-environments'))).expanduser()
    config = json.loads((args.root / 'deploy/product.json').read_text())
    if args.action in ('staging', 'production'):
        project = config.get(args.action + '_project', 'agbot-' + args.action)
        image = args.image if args.image is not None else selected_image(state_root / project)
        result = validate_production_image(image)
    elif args.action == 'invalidate':
        (state_root / config.get('integration_project', 'agbot-integration') / 'build-receipt.json').unlink(missing_ok=True)
        result = None
    else:
        state = state_root / config.get('integration_project', 'agbot-integration')
        result = (record_build if args.action == 'record' else verify_deployment)(args.root, state)
    if result:
        print(result)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, IndexError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        print(f'release-guard: {error}', file=sys.stderr)
        sys.exit(1)
