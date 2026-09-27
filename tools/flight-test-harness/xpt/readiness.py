"""Bounded startup diagnosis and actual plugin-load evidence."""
from pathlib import Path
import re

class SimulatorStartupError(RuntimeError):pass

def log_state(path):
    text=Path(path).read_text(encoding='utf-8',errors='replace') if Path(path).exists() else ''
    fatal=[line for line in text.splitlines() if any(s in line for s in (
        'E/SYS: MACIBM_alert:', '--=={This application has crashed!', 'Vulkan device loss',
        'Encountered Vulkan device loss', 'Error generating path for aircraft preview'))
        or re.search(r'I/SYS:.*\*\*.*SAFE MODE',line)]
    return {'fatal':fatal[-16:],
            'loaded_plugins':[line for line in text.splitlines() if line.startswith('Loaded:')],
            'plugin_failures':[line for line in text.splitlines() if re.search(r'(Error Code =|Failed to load.*plugin|could not be loaded)',line,re.I)],
            'openxr_errors':[line for line in text.splitlines() if 'XR_ERROR_' in line],
            'cache':[line for line in text.splitlines() if 'TEX_cache ready' in line]}

def check_startup(root,minimum_epoch=0):
    path=Path(root)/'Log.txt'
    if path.exists() and path.stat().st_mtime<minimum_epoch:
        return {'fatal':[],'previous_process_log':True}
    state=log_state(path)
    if state['fatal']:raise SimulatorStartupError('\n'.join(state['fatal']))
    return state

def startup_menu_visible(api):
    # A failed resume is not evidence that opening/toggling the aircraft browser
    # is appropriate. Only a discovered, explicit UI state permits recovery.
    for name in ('sim/graphics/view/main_menu_visible','sim/operation/main_menu_visible'):
        if name in api.datarefs:return api.get_scalar(name)==1
    return False
