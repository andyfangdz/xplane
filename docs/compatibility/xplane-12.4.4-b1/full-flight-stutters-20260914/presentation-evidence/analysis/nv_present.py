"""Read and precisely restore X-Plane's one public NVIDIA DRS presentation setting.

ABI and interface IDs from pinned official NVIDIA/nvapi headers in ./nvapi.
Never modifies global profiles or creates application/profile bindings.
"""
import argparse
import base64
import ctypes as c
import hashlib
import json
from pathlib import Path
import re
import sys
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parent
APP = 'D:/X-Plane 12/X-Plane.exe'
SETTING = 0x20D690F8
U32, HANDLE = c.c_uint32, c.c_void_p
WSTRING = c.c_wchar * 2048

class Binary(c.Structure):
    _fields_ = [('length', U32), ('data', c.c_ubyte * 4096)]

class Value(c.Union):
    _pack_ = 4
    _fields_ = [('dword', U32), ('binary', Binary), ('text', WSTRING), ('qword', c.c_uint64)]

class Setting(c.Structure):
    _pack_ = 4
    _fields_ = [('version', U32), ('name', WSTRING), ('id', U32), ('type', U32),
                ('location', U32), ('predefined', U32), ('predefined_valid', U32),
                ('default', Value), ('current', Value)]

class Application(c.Structure):
    _fields_ = [('version', U32), ('predefined', U32), ('name', WSTRING),
                ('friendly', WSTRING), ('launcher', WSTRING), ('file_in_folder', WSTRING),
                ('flags', U32), ('command_line', WSTRING)]

class SettingValues(c.Structure):
    _pack_ = 4
    _fields_ = [('version', U32), ('count', U32), ('type', U32), ('default', Value), ('values', Value * 100)]

class Profile(c.Structure):
    _fields_ = [('version', U32), ('name', WSTRING), ('gpu_support', U32),
                ('predefined', U32), ('app_count', U32), ('setting_count', U32)]

def new(kind, version=1):
    value = kind()
    value.version = c.sizeof(kind) | (version << 16)
    return value

def atomic_json(path, value):
    path = Path(path)
    tmp = path.with_suffix(path.suffix + '.tmp')
    tmp.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')
    tmp.replace(path)

def describe(s):
    if s is None:
        return None
    return dict(id=f'0x{s.id:08x}', name=s.name, type=s.type, location=s.location,
                is_current_predefined=s.predefined, is_predefined_valid=s.predefined_valid,
                current=s.current.dword, default=s.default.dword)

class NvError(RuntimeError):
    def __init__(self, name, code, message):
        self.code = code
        super().__init__(f'{name}: {code}: {message}')

class DRS:
    def __init__(self):
        assert c.sizeof(c.c_wchar) == 2 and c.sizeof(Setting) == 12320
        headers = ROOT / 'nvapi'
        source = json.loads((headers / 'source.json').read_text(encoding='utf-8-sig'))
        for item in source['files']:
            assert hashlib.sha256((headers/item['file']).read_bytes()).hexdigest().lower() == item['sha256'].lower()
        self.ids = dict((n, int(v,16)) for n,v in re.findall(r'\{ "(NvAPI_\w+)", (0x[0-9a-f]+) \}', (headers/'nvapi_interface.h').read_text()))
        self.dll = c.WinDLL('C:/Windows/System32/nvapi64.dll')
        self.query = self.dll.nvapi_QueryInterface
        self.query.argtypes = [U32]
        self.query.restype = HANDLE
        self.call('NvAPI_Initialize', [], [])
        self.session = HANDLE()
        self.call('NvAPI_DRS_CreateSession', [c.POINTER(HANDLE)], [c.byref(self.session)])
        self.call('NvAPI_DRS_LoadSettings', [HANDLE], [self.session])
        self.profile = HANDLE()
        self.app = new(Application, 4)
        self.call('NvAPI_DRS_FindApplicationByName', [HANDLE, c.c_wchar_p, c.POINTER(HANDLE), c.POINTER(Application)],
                  [self.session, APP, c.byref(self.profile), c.byref(self.app)])
        self.info = new(Profile)
        self.call('NvAPI_DRS_GetProfileInfo', [HANDLE,HANDLE,c.POINTER(Profile)], [self.session,self.profile,c.byref(self.info)])
        if self.info.name.lower() not in ('x-plane', 'x-plane 12', 'x-plane.exe'):
            raise RuntimeError(f'Unexpected application profile: {self.info.name!r}; read-only review required')
        self.apps = (Application * self.info.app_count)()
        for app in self.apps:
            app.version = c.sizeof(Application) | (4 << 16)
        count = U32(self.info.app_count)
        self.call('NvAPI_DRS_EnumApplications', [HANDLE,HANDLE,U32,c.POINTER(U32),c.POINTER(Application)],
                  [self.session,self.profile,0,c.byref(count),self.apps])
        self.app_names = [a.name for a in self.apps[:count.value]]

    def call(self, name, types, args):
        address = self.query(self.ids[name])
        if not address:
            raise RuntimeError(f'{name} unavailable')
        code = c.WINFUNCTYPE(c.c_int, *types)(address)(*args)
        if code:
            msg = c.create_string_buffer(64)
            c.WINFUNCTYPE(c.c_int,c.c_int,c.c_char_p)(self.query(self.ids['NvAPI_GetErrorMessage']))(code,msg)
            raise NvError(name, code, msg.value.decode(errors='replace'))

    def setting(self):
        s = new(Setting)
        try:
            self.call('NvAPI_DRS_GetSetting',[HANDLE,HANDLE,U32,c.POINTER(Setting)], [self.session,self.profile,SETTING,c.byref(s)])
        except NvError as e:
            if e.code == -160:
                return None
            raise
        assert s.id == SETTING and s.type == 0
        return s

    def snapshot(self):
        s = self.setting()
        available = new(SettingValues)
        max_values = U32(100)
        try:
            self.call('NvAPI_DRS_EnumAvailableSettingValues',[U32,c.POINTER(U32),c.POINTER(SettingValues)],
                      [SETTING,c.byref(max_values),c.byref(available)])
            availability = dict(type=available.type, default=available.default.dword,
                                values=[v.dword for v in available.values[:max_values.value]])
        except NvError as e:
            availability = dict(error=str(e))
        count = U32(self.info.setting_count)
        settings = (Setting * count.value)()
        for item in settings:
            item.version = c.sizeof(Setting) | (1 << 16)
        self.call('NvAPI_DRS_EnumSettings',[HANDLE,HANDLE,U32,c.POINTER(U32),c.POINTER(Setting)],
                  [self.session,self.profile,0,c.byref(count),settings])
        return dict(utc=datetime.now(timezone.utc).isoformat(),app=APP,profile=self.info.name,
                    applications=self.app_names,setting=describe(s),availability=availability,
                    setting_bytes=base64.b64encode(bytes(s)).decode() if s is not None else None,
                    profile_settings=[describe(x) for x in settings[:count.value]])

    def set(self, value):
        expected_apps = {'x-plane.exe','plane maker.exe','plane-maker.exe','airfoil maker.exe','airfoil-maker.exe'}
        if set(a.lower() for a in self.app_names) != expected_apps:
            raise RuntimeError(f'Profile shared with unexpected applications: {self.app_names}')
        s = new(Setting)
        s.id, s.type, s.current.dword = SETTING, 0, value
        self.call('NvAPI_DRS_SetSetting',[HANDLE,HANDLE,c.POINTER(Setting)], [self.session,self.profile,c.byref(s)])
        self.call('NvAPI_DRS_SaveSettings',[HANDLE],[self.session])

    def restore(self, before, expected):
        if before['app'] != APP or before['profile'] != self.info.name or before['applications'] != self.app_names:
            raise RuntimeError('Profile identity changed; refusing restore')
        cur = describe(self.setting())
        if cur == before['setting']:
            return
        if cur is None or cur['current'] != expected or cur['location'] != 0 or cur['is_current_predefined']:
            raise RuntimeError('Setting changed outside this experiment; refusing to overwrite it')
        prior = Setting.from_buffer_copy(base64.b64decode(before['setting_bytes'])) if before['setting_bytes'] else None
        assert prior is None or (prior.id == SETTING and prior.type == 0)
        if prior is not None and prior.location == 0 and not prior.predefined:
            self.call('NvAPI_DRS_SetSetting',[HANDLE,HANDLE,c.POINTER(Setting)], [self.session,self.profile,c.byref(prior)])
        else:
            self.call('NvAPI_DRS_DeleteProfileSetting',[HANDLE,HANDLE,U32],[self.session,self.profile,SETTING])
        self.call('NvAPI_DRS_SaveSettings',[HANDLE],[self.session])

    def close(self):
        self.call('NvAPI_DRS_DestroySession',[HANDLE],[self.session])
        self.call('NvAPI_Unload',[],[])

def main():
    p = argparse.ArgumentParser()
    p.add_argument('action', choices=['read','apply','restore'])
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--before', type=Path)
    p.add_argument('--value',type=int,choices=[0,1,2])
    args = p.parse_args()
    drs = DRS()
    try:
        snap = drs.snapshot()
        if args.action == 'read':
            atomic_json(args.output,snap)
        elif args.action == 'apply':
            assert args.before and args.value is not None
            if args.before.exists():
                raise RuntimeError('Refusing to overwrite recovery snapshot')
            atomic_json(args.before,snap)
            drs.set(args.value)
        else:
            assert args.before and args.value is not None
            before = json.loads(args.before.read_text(encoding='utf-8'))
            drs.restore(before,args.value)
    finally:
        drs.close()
    if args.action != 'read':
        verify = DRS()
        try:
            snap = verify.snapshot()
            atomic_json(args.output,snap)
            if args.action == 'apply':
                assert snap['setting']['current'] == args.value and snap['setting']['location'] == 0
            else:
                assert snap['setting'] == before['setting'], 'Restoration readback mismatch'
                assert snap['profile_settings'] == before['profile_settings'], 'Profile settings changed'
        finally:
            verify.close()
    print(json.dumps({k:v for k,v in snap.items() if k not in ('setting_bytes','profile_settings')},indent=2))

if __name__ == '__main__':
    main()
