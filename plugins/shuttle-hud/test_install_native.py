"""Exercise the panel/optics bindings with a miniature original aircraft."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from PIL import Image

import install_native as hud


class GeneratedHudTest(unittest.TestCase):
    def test_paired_unkeyed_screens_feed_only_the_collimated_3d_region(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, output = root / 'original', root / 'package'
            for path in [source / 'COCKPIT/-PANELS-', source / 'Objects', output / 'Objects']:
                path.mkdir(parents=True)
            sample_panel = 'but_HUD_power but_HUD_power\n  POS 200 300\n\n'
            keys = ['acf/_name', 'acf/_g1000_keyboard_EQ', 'acf/_hud_region_index',
                    'acf/_cockpit_type', 'acf/_elev1_dn', 'acf/_use_cus_gear_damping',
                    '_obja/18/_obj_flags', '_obja/18/_v10_att_file_stl']
            for side in ['left', 'right', 'top', 'bottom']:
                keys.extend([f'acf/_hud_fov_{side}_rat', f'acf/_hud_panel_{side}_px'])
            for i in range(3):
                keys.extend([f'acf/_pe_xyz/{i}', f'_gear/{i}/_damp'])
            for i in [0, 35]:
                keys.append(f'_obja/{i}/_v10_att_file_stl')
            acf = ''.join(f'P {key} 1\n' for key in keys)
            acf += f'PANEL_2D_BEGIN\n{sample_panel}PANEL_2D_END\nPANEL_3D_BEGIN\nPANEL_3D_END\n'
            (source / 'Orbiter_Glider.acf').write_text(acf)
            (source / 'Orbiter_Cockpit.obj').write_text('POINT_COUNTS 1 0 0 0\nTRIS 1182 12\n')
            (source / 'Objects/Shuttle_Transp.obj').write_text('TRIS\t0 4902\n')
            Image.new('RGBA', (2048, 2048)).save(source / 'COCKPIT/-PANELS-/panel.png')
            binary = root / 'fixture.dll'
            binary.write_bytes(b'fixture')
            hud.transform(source, output, binary)
            self.assertEqual((source / 'Orbiter_Glider.acf').read_text(), acf)
            generated = (output / 'Orbiter_Glider.acf').read_text()
            self.assertIn('P acf/_g1000_keyboard_EQ 3\n', generated)
            self.assertIn(f'PANEL_2D_BEGIN\n{sample_panel}PANEL_2D_END', generated)
            panel = generated.split('PANEL_3D_BEGIN\n')[1].split('PANEL_3D_END')[0]
            self.assertEqual(panel.count('g1000_PFD '), 1)
            self.assertEqual(panel.count('g1000_MFD '), 1)
            pfd, mfd = panel.split('g1000_PFD ')[1].split('g1000_MFD ')
            self.assertIn('SHOW_EQUAL -1.000000 fsim_hud/plugin_enabled[0]', pfd)
            self.assertIn('SHOW_EQUAL 1.000000 fsim_hud/plugin_enabled[0]', mfd)
            self.assertIn('BUS_SRC 1', mfd)
            x, y = (float(word) for word in mfd.split('POS ')[1].splitlines()[0].split())
            def source_edge(name):
                key = f'P acf/_hud_panel_{name}_px '
                return float(generated.split(key)[1].splitlines()[0])
            self.assertEqual((x - 512, y + 256 - 384), (source_edge('left'), source_edge('bottom')))
            self.assertEqual((source_edge('right') - source_edge('left'),
                              source_edge('top') - source_edge('bottom')), (640, 640))
            self.assertEqual((output / 'Objects/Shuttle_FSim_Transp.obj').read_text().count('ATTR_hud_glass'), 2)


if __name__ == '__main__':
    unittest.main()
