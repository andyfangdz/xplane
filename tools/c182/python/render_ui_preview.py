"""Rasterize asset-free egui preview meshes (requires numpy and Pillow)."""
import json
from pathlib import Path
import sys
import numpy as np
from PIL import Image

def render(path):
    data = json.loads(path.read_text())
    width, height = data['size']
    canvas = np.zeros((height, width, 4), dtype=np.float32)
    textures = {}
    for item in data['textures']:
        w, h = item['size']
        pixels = np.asarray(item['pixels'], np.float32).reshape(h, w, 4) / 255
        if item['pos'] is None:
            textures[item['id']] = pixels
        else:
            x, y = item['pos']
            textures[item['id']][y:y+h, x:x+w] = pixels
    for mesh in data['meshes']:
        vertices = np.asarray(mesh['vertices'], np.float32)
        texture = textures[mesh['texture']]
        th, tw, _ = texture.shape
        clip = mesh['clip']
        for indices in np.asarray(mesh['indices']).reshape(-1, 3):
            v = vertices[indices]
            left = max(0, int(np.floor(v[:, 0].min())), int(np.ceil(clip[0])))
            right = min(width, int(np.ceil(v[:, 0].max())), int(np.floor(clip[2])))
            top = max(0, int(np.floor(v[:, 1].min())), int(np.ceil(clip[1])))
            bottom = min(height, int(np.ceil(v[:, 1].max())), int(np.floor(clip[3])))
            if right <= left or bottom <= top:
                continue
            x, y = np.meshgrid(np.arange(left, right)+.5, np.arange(top, bottom)+.5)
            a, b, c = v[:, :2]
            den = (b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
            if abs(den) < 1e-6:
                continue
            u = ((b[1]-c[1])*(x-c[0])+(c[0]-b[0])*(y-c[1]))/den
            q = ((c[1]-a[1])*(x-c[0])+(a[0]-c[0])*(y-c[1]))/den
            weights = np.stack([u, q, 1-u-q], -1)
            mask = (weights >= -1e-6).all(-1)
            if not mask.any():
                continue
            attrs = weights @ v[:, 2:]
            tx = np.clip(attrs[..., 0]*tw-.5, 0, tw-1)
            ty = np.clip(attrs[..., 1]*th-.5, 0, th-1)
            ix, iy = tx.astype(int), ty.astype(int)
            fx, fy = (tx-ix)[..., None], (ty-iy)[..., None]
            texel = ((1-fx)*texture[iy, ix]+fx*texture[iy, np.minimum(ix+1, tw-1)])*(1-fy)+((1-fx)*texture[np.minimum(iy+1, th-1), ix]+fx*texture[np.minimum(iy+1, th-1), np.minimum(ix+1, tw-1)])*fy
            color = texel*attrs[..., 2:]/255
            region = canvas[top:bottom, left:right]
            region[mask] = (color+region*(1-color[..., 3:4]))[mask]
    result = path.with_suffix('.png')
    Image.fromarray(np.uint8(np.clip(canvas, 0, 1)*255)).save(result)
    print(result)

for source in sorted(Path(sys.argv[1]).glob('*.json')):
    render(source)
