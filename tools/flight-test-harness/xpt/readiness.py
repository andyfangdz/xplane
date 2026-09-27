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

UI_REFS = ('sim/graphics/view/window_width','sim/graphics/view/window_height','sim/time/total_running_time_sec')

class UiReadiness:
    """The Web API answers before the UI finishes booting. On 12.4.4 beta 1, a
    /flight request between 'Accepting receipt' and 'Showing subscreen Main
    Menu' loaded without a primary window or first frame and never resumed
    (three sessions). Log.txt is buffered, and the main-menu catalog lacks the
    window and time datarefs, so require continuous API responsiveness for a
    settle interval and retain catalog sizes and any UI readbacks."""
    def __init__(self, required=20):
        self.required=required;self.samples=[];self.consecutive=0
    def observe(self, api, epoch):
        api.refresh_catalogs()
        sample={'epoch':epoch,'datarefs':len(api.datarefs),'commands':len(api.commands)}
        present=[name for name in UI_REFS if name in api.datarefs]
        if present:
            sample['readback']=api.get_batch(present)
        self.consecutive+=1
        self.samples.append(sample)
        return self.consecutive>=self.required

def startup_menu_visible(api):
    # A failed resume is not evidence that opening/toggling the aircraft browser
    # is appropriate. Only a discovered, explicit UI state permits recovery.
    for name in ('sim/graphics/view/main_menu_visible','sim/operation/main_menu_visible'):
        if name in api.datarefs:return api.get_scalar(name)==1
    return False
