"""Optional native AVI and WASAPI evidence; never writes flight guidance."""
import json,shutil,subprocess,sys,time
from pathlib import Path
from .config import ROOT,atomic_json

class VideoCapture:
    def __init__(self,api,path,xplane_root):
        self.api,self.path=api,path
        self.root=Path(xplane_root)
        self.process=None;self.recording=False;self.started=False
        self.log={'schema_version':5,'view':'forward_with_nothing','hud':'native custom HUD v5',
                  'tapes_hsi':'Garmin G1000 moving tapes, drums, trends, heading-up compass and GPS CDI',
                  'flight_director':'Reference-matched G1000 shallow magenta command bars, faceted yellow aircraft, solid pitch marks and bank arc'}
        self.before=set()

    def movies(self):
        result=set()
        for folder in [self.root,self.root/'Output',self.root/'Output/movies',self.root/'Output/screenshots']:
            result.update(folder.glob('*.avi'))
        return result

    def prepare(self):
        if self.api.get_scalar('sim/graphics/VR/enabled'):
            self.api.command('sim/VR/disable_vr',.2)
            deadline=time.monotonic()+15
            while self.api.get_scalar('sim/graphics/VR/enabled'):
                if time.monotonic()>deadline:raise RuntimeError('Unable to establish 2D recording view')
                time.sleep(.2)
        self.log['vr_enabled_readback']=self.api.get_scalar('sim/graphics/VR/enabled')
        names=['sim/operation/sound/sound_on']+['sim/operation/sound/'+g+'_volume_ratio' for g in ['master','engine','prop','interior','exterior','warning']]
        self.api.require_datarefs(names)
        targets={n:(.6 if n.endswith('master_volume_ratio') else 1) for n in names}
        for n,v in targets.items():self.api.set_dataref(n,v)
        self.log['sound_readback']=self.api.get_batch(names)
        if any(abs(float(v)-targets[n])>.02 for n,v in self.log['sound_readback'].items()):raise RuntimeError('Sound readiness failed')
        self.api.require_datarefs(['xpt/video_hud/version','xpt/video_hud/navigation_ready','xpt/video_hud/rust_implementation'])
        if self.api.get_scalar('xpt/video_hud/rust_implementation')!=1:raise RuntimeError('Rust HUD implementation missing')
        if self.api.get_scalar('xpt/video_hud/version')!=5:raise RuntimeError('Native custom HUD v5 missing')
        self.api.command('xpt/video_hud/load_approach',.2)
        deadline=time.monotonic()+12
        while time.monotonic()<deadline:
            self.api.command('xpt/video_hud/activate_approach',.2)
            if self.api.get_scalar('xpt/video_hud/navigation_ready')==1:break
            time.sleep(.4)
        else:raise RuntimeError('Native KOLLI to RW22 approach unavailable')
        self.api.set_dataref('sim/cockpit2/radios/actuators/HSI_source_select_pilot',2)
        nav=self.root/'Aircraft/X-Aviation/TorqueSim SR20/plugins/XPTVideoHUD/navigation-readback.txt'
        shutil.copy2(nav,self.path/'navigation-readback.txt')
        self.api.set_dataref('sim/graphics/view/field_of_view_deg',75)
        for axis in ['psi','the','phi']:
            self.api.set_dataref('sim/graphics/view/pilots_head_'+axis,0)
        for n in ['alpha_rect','alpha_joys','plus_size']:
            name='sim/private/controls/mouse_yoke/'+n
            if name in self.api.datarefs:self.api.set_dataref(name,0)
        # Head-angle writes can switch X-Plane back into the 3D cockpit. Select
        # the unobstructed view last, after every camera setting.
        self.api.command('sim/view/forward_with_nothing',.2)
        self.before=self.movies()
        script=ROOT/'scripts/Capture-WasapiLoopback.py'
        args=[sys.executable,str(script),'--output',str(self.path/'audio.wav'),
              '--stop-file',str(self.path/'audio.stop'),'--ready-file',str(self.path/'audio.ready.json'),
              '--metadata',str(self.path/'audio.metadata.json')]
        self.audio_log=(self.path/'audio-capture.log').open('w')
        self.process=subprocess.Popen(args,creationflags=subprocess.CREATE_NO_WINDOW,stdout=self.audio_log,stderr=subprocess.STDOUT)
        deadline=time.monotonic()+12
        while not (self.path/'audio.ready.json').exists():
            if self.process.poll() is not None:raise RuntimeError('Audio recorder exited before readiness')
            if time.monotonic()>deadline:raise RuntimeError('Audio readiness timeout')
            time.sleep(.1)
        self.log['audio_ready']=json.loads((self.path/'audio.ready.json').read_text())
        atomic_json(self.path/'capture.json',self.log)

    def observe(self,s):
        if self.started or s['phase']!='downwind' or s['runway_along_ft']>2800:return
        if abs(s['ias_kias']-100)>2 or abs(s['agl_ft']-1000)>35:raise RuntimeError('Video entry gate failed')
        self.log['hud_readback']=self.api.get_batch(['xpt/video_hud/rust_implementation','xpt/video_hud/version','xpt/video_hud/draw_frames',
            'xpt/video_hud/font_ready','xpt/video_hud/navigation_ready',
            'sim/cockpit/radios/gps_course_degtm','sim/cockpit/radios/gps_fromto',
            'sim/graphics/view/view_type','sim/graphics/view/field_of_view_deg',
            'sim/cockpit2/gauges/indicators/airspeed_kts_pilot','sim/cockpit2/gauges/indicators/altitude_ft_pilot',
            'sim/cockpit2/gauges/indicators/vvi_fpm_pilot','sim/cockpit2/gauges/indicators/heading_AHARS_deg_mag_pilot',
            'sim/cockpit2/gauges/indicators/ground_track_mag_pilot','sim/cockpit2/autopilot/heading_dial_deg_mag_pilot',
            'sim/cockpit2/autopilot/altitude_dial_ft','sim/cockpit2/gauges/actuators/barometer_setting_in_hg_pilot',
            'sim/cockpit/radios/gps_hdef_dot','sim/cockpit/radios/gps_hdef_nm_per_dot',
            'sim/cockpit/radios/gps_cdi_sensitivity','sim/cockpit/radios/gps_sequencing',
            'sim/aircraft/view/acf_Vso','sim/aircraft/view/acf_Vs','sim/aircraft/view/acf_Vfe',
            'sim/aircraft/view/acf_Vno','sim/aircraft/view/acf_Vne','xpt/video_hud/full_flap_limit_kias'])
        h=self.log['hud_readback']
        if not h['sim/aircraft/view/acf_Vso']<h['xpt/video_hud/full_flap_limit_kias']<=h['sim/aircraft/view/acf_Vfe']:
            raise RuntimeError('Native full-flap airspeed marking unavailable')
        shutil.copy2(self.root/'Aircraft/X-Aviation/TorqueSim SR20/plugins/XPTVideoHUD/instrument-readback.txt',self.path/'instrument-readback.txt')
        if (h['sim/graphics/view/view_type']!=1024 or h['xpt/video_hud/font_ready']!=1 or h['xpt/video_hud/draw_frames']<30 or
            h['xpt/video_hud/navigation_ready']!=1 or not 220<h['sim/cockpit/radios/gps_course_degtm']<224):
            raise RuntimeError('Native HUD or GPS not ready at recording gate')
        self.log['movie_start_epoch']=time.time()
        self.log['movie_start_sim_time']=s['sim_time']
        self.api.command('sim/operation/video_record_toggle',.2)
        self.recording=self.started=True
        atomic_json(self.path/'capture.json',self.log)

    def stop(self):
        errors=[]
        if self.recording:
            try:
                self.log['movie_stop_epoch']=time.time()
                self.api.command('sim/operation/video_record_toggle',.2)
                self.log['movie_stop_sim_time']=self.api.get_scalar('sim/time/total_flight_time_sec')
                time.sleep(.5)
            except Exception as e:errors.append(str(e))
            self.recording=False
        if self.process:
            (self.path/'audio.stop').write_text('stop')
            try:self.log['audio_exit_code']=self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.process.kill();self.process.wait();errors.append('Audio stop timeout')
            self.audio_log.close()
        new=sorted(self.movies()-self.before,key=lambda p:p.stat().st_ctime)
        self.log['native_movies']=[]
        for i,p in enumerate(new):
            dest=self.path/f'native-{i+1:02d}.avi'
            try:shutil.move(str(p),str(dest));self.log['native_movies'].append(str(dest))
            except Exception as e:errors.append(str(e))
        self.log['errors']=errors
        atomic_json(self.path/'capture.json',self.log)
