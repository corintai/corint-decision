"""Verify database registration ordering, scoped cleanup, and service restoration."""
from contextlib import closing, redirect_stdout, redirect_stderr
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('replay_local', ROOT/'scripts/replay_local.py')
LOCAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LOCAL)


class ReplayLocalTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        verification = patch.object(LOCAL, 'verify_repository')
        self.verify = verification.start()
        self.addCleanup(verification.stop)
        self.path = Path(self.temp.name)
        self.database = self.path/'credentials.sqlite'
        self.original = {'principal': {'id': 'existing_admin', 'platform_admin': True}, 'digest': [1]*32}
        with closing(sqlite3.connect(self.database)) as connection, connection:
            connection.execute('CREATE TABLE tenant_credentials(id INTEGER PRIMARY KEY, revision INTEGER, document TEXT)')
            connection.execute('CREATE TABLE tenant_credential_audit(revision INTEGER PRIMARY KEY,actor TEXT,action TEXT,principal_id TEXT,occurred_at_ms INTEGER)')
            connection.execute('INSERT INTO tenant_credentials VALUES(1,0,?)',
                               (json.dumps({'entries': {'existing_admin': self.original}}),))
        self.scope = {'tenant_id':'local','environment':'demo','deployment':'default'}
        self.config = self.path/'auth.json'
        self.config.write_text(json.dumps({'scope':self.scope,'control_store':{'type':'sqlite','path':'credentials.sqlite'}}))
        self.repository = self.path/'test-policies'
        self.repository.mkdir()
        (self.repository/'registry.yaml').write_text('version: "0.1"\nregistry: []\n')
        self.args = SimpleNamespace(run_dir=self.path/'run', server_bin=Path(sys.executable), auth_config=None,
                                    token_env='CORINT_DECISION_TOKEN', credential_ttl=3600,
                                    repository=self.repository,
                                    url='http://localhost:8081/v1/decide')
        self.state = {'pid':123,'url':'http://127.0.0.1:8081'}
        self.calls=[]
        self.services=SimpleNamespace(running=lambda name:self.state, process_details=lambda pid:(Path(sys.executable),ROOT),
            process_configuration=lambda pid:([sys.executable],{'CORINT_AUTH_CONFIG':str(self.config)}),
            stop=self.stop, start=self.start)

    def registry(self):
        with closing(sqlite3.connect(self.database)) as connection, connection:
            return json.loads(connection.execute('SELECT document FROM tenant_credentials').fetchone()[0])['entries']

    def stop(self, name):
        self.calls.append('stop')
        self.state=None

    def start(self, name, command, environment):
        self.calls.append('start')
        self.assertIn('corint_decision_server=info', environment['RUST_LOG'])
        entries=self.registry()
        if len(entries)==2:
            self.assertEqual(environment['CORINT_REPOSITORY_PATH'],str(self.repository.resolve()))
            principal_id=next(key for key in entries if key.startswith('csv_replay_'))
            entry=entries[principal_id]
            token=os.environ['CORINT_DECISION_TOKEN']
            self.assertEqual(entry['digest'],list(hashlib.sha256(token.encode()).digest()))
            self.assertFalse(entry['principal']['platform_admin'])
            self.assertEqual(entry['principal']['grants'],[{'scope':self.scope,'permissions':['decide']}])
            self.assertNotIn(token,json.dumps(entries))
            self.assertNotIn('CORINT_DECISION_TOKEN',environment)
        else:
            self.assertNotIn('CORINT_REPOSITORY_PATH',environment)
            self.assertEqual(entries,{'existing_admin':self.original})
        self.state={'pid':124,'url':'http://127.0.0.1:8081'}
        return True

    def test_register_before_start_and_remove_after_stop_preserves_original_service(self):
        with patch.object(LOCAL,'services_module',return_value=self.services), patch.dict(os.environ,clear=True), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            with LOCAL.local_decision(self.args):
                self.calls.append('replay')
                self.assertEqual(len(self.registry()),2)
                self.assertIn('CORINT_DECISION_TOKEN',os.environ)
            self.assertNotIn('CORINT_DECISION_TOKEN',os.environ)
        self.assertEqual(self.calls,['stop','start','replay','stop','start'])
        self.assertEqual(self.registry(),{'existing_admin':self.original})
        with closing(sqlite3.connect(self.database)) as connection, connection:
            self.assertEqual(connection.execute('SELECT action FROM tenant_credential_audit ORDER BY revision').fetchall(),[('create',),('revoke',)])

    def test_failure_and_interrupt_cleanup_restore_existing_environment(self):
        for error in (ValueError('request failed'),KeyboardInterrupt()):
            with self.subTest(error=type(error).__name__), patch.object(LOCAL,'services_module',return_value=self.services), patch.dict(os.environ,{'CORINT_DECISION_TOKEN':'original-token'}), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                with self.assertRaises(type(error)):
                    with LOCAL.local_decision(self.args):
                        raise error
                self.assertEqual(os.environ['CORINT_DECISION_TOKEN'],'original-token')
                self.assertEqual(self.registry(),{'existing_admin':self.original})
                self.assertIsNotNone(self.state)

    def test_failed_start_removes_credential_and_restores_original_service(self):
        original_start=self.services.start
        def start(*args):
            if len(self.registry())==2:
                raise RuntimeError('startup failed')
            return original_start(*args)
        self.services.start=start
        with patch.object(LOCAL,'services_module',return_value=self.services), patch.dict(os.environ,clear=True), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError,'startup failed'):
                with LOCAL.local_decision(self.args):
                    self.fail('must not replay')
            self.assertNotIn('CORINT_DECISION_TOKEN',os.environ)
        self.assertEqual(self.registry(),{'existing_admin':self.original})
        self.assertIsNotNone(self.state)

    def test_initially_stopped_service_stays_stopped(self):
        self.state=None
        self.args.auth_config=self.config
        with patch.object(LOCAL,'services_module',return_value=self.services), patch.dict(os.environ,clear=True), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            with LOCAL.local_decision(self.args):
                pass
        self.assertEqual(self.calls,['start','stop'])
        self.assertIsNone(self.state)

    def test_cleanup_preserves_other_changes_made_during_replay(self):
        LOCAL.change_credential(self.database,'csv_replay_example',{'example':True})
        LOCAL.change_credential(self.database,'concurrent_principal',{'different':True})
        LOCAL.change_credential(self.database,'csv_replay_example')
        self.assertEqual(set(self.registry()),{'existing_admin','concurrent_principal'})

    def test_repository_preflight_failure_does_not_stop_service_or_register_token(self):
        self.verify.side_effect = ValueError('wrong repository')
        with patch.object(LOCAL,'services_module',return_value=self.services), patch.dict(os.environ,clear=True), redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(ValueError,'wrong repository'):
                with LOCAL.local_decision(self.args):
                    self.fail('must not start')
            self.assertNotIn('CORINT_DECISION_TOKEN',os.environ)
        self.assertEqual(self.calls,[])
        self.assertEqual(self.registry(),{'existing_admin':self.original})

    def test_database_outside_auth_directory_is_rejected(self):
        self.config.write_text(json.dumps({'scope':self.scope,'control_store':{'type':'sqlite','path':'../other.sqlite'}}))
        with self.assertRaises((ValueError,FileNotFoundError)):
            LOCAL.credential_database(self.config)


if __name__=='__main__':
    unittest.main()
