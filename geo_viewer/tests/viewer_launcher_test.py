import importlib.machinery
import importlib.util
import json
import os
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from unittest import mock

loader = importlib.machinery.SourceFileLoader('viewer_launcher', str(Path(__file__).resolve().parents[2] / 'scripts' / 'viewer'))
spec = importlib.util.spec_from_loader(loader.name, loader)
launcher = importlib.util.module_from_spec(spec)
loader.exec_module(launcher)


class Handler(BaseHTTPRequestHandler):
    requests = []

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', '0')))
        self.requests.append((self.path, self.headers.get('Authorization'), body))
        if self.path == '/api/portal/login' and json.loads(body).get('access_code') == 'redirect':
            self.send_response(302)
            self.send_header('Location', '/unexpected')
            self.end_headers()
            return
        if self.path == '/api/portal/login':
            code = json.loads(body).get('access_code')
            token = {'empty': '', 'number': 123, 'newline': 'one\ntwo', 'large': 'x' * 70_000}.get(code, 'test-session')
            data = json.dumps([] if code == 'array' else {'token': token}).encode()
        else:
            data = b'{}'
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_args):
        pass


class ViewerLauncherTest(unittest.TestCase):
    def test_login_and_logout_use_the_correct_session(self):
        Handler.requests = []
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            base = f'http://127.0.0.1:{server.server_port}'
            token = launcher.login(base, 'private-code')
            launcher.logout(base, token)
            self.assertEqual(token, 'test-session')
            self.assertEqual(Handler.requests[0][0], '/api/portal/login')
            self.assertEqual(json.loads(Handler.requests[0][2]), {'access_code': 'private-code'})
            self.assertEqual(Handler.requests[1][1], 'Bearer test-session')
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_insecure_remote_or_credential_urls_are_rejected(self):
        for base in ('http://192.0.2.1:8080', 'http://user:password@localhost:8080', 'file:///tmp/a'):
            with self.subTest(base=base), self.assertRaises(ValueError):
                launcher.validate_base(base)

    def test_login_refuses_redirects(self):
        Handler.requests = []
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with self.assertRaises(launcher.error.HTTPError):
                launcher.login(f'http://127.0.0.1:{server.server_port}', 'redirect')
            self.assertEqual(len(Handler.requests), 1)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_login_rejects_invalid_session_tokens(self):
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            for code in ('empty', 'number', 'newline', 'large', 'array'):
                with self.subTest(code=code), self.assertRaises(ValueError):
                    launcher.login(f'http://127.0.0.1:{server.server_port}', code)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_launch_passes_session_and_revokes_it_on_exit(self):
        Handler.requests = []
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as temp:
                binary = Path(temp) / 'viewer'
                binary.touch()
                base = f'http://127.0.0.1:{server.server_port}'
                with mock.patch.dict(os.environ, {'AGBOT_VIEWER_BIN': str(binary), 'GEO_HUB_URL': base}), \
                     mock.patch.object(launcher.getpass, 'getpass', return_value='private-code'), \
                     mock.patch.object(launcher.sys, 'argv', ['viewer', '--check-connection']), \
                     mock.patch.object(launcher.subprocess, 'call', return_value=0) as child:
                    self.assertEqual(launcher.main(), 0)
                self.assertEqual(child.call_args.args[0], [str(binary), '--check-connection'])
                self.assertEqual(child.call_args.kwargs['env']['GEO_VIEWER_SESSION_TOKEN'], 'test-session')
                self.assertEqual([item[0] for item in Handler.requests], ['/api/portal/login', '/api/portal/logout'])
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


if __name__ == '__main__':
    unittest.main()
