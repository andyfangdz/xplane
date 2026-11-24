# Aircraft text overlays

`line_overlay.py` records edits to text assets such as ACF, OBJ8, SND and airfoil files. The recipe contains added bytes, references to unmodified lines, and the SHA-256 of the required original. Git computes the diff when recording. Applying an existing recipe needs only Python 3.9 or newer.

```powershell
python line_overlay.py record original.obj edited.obj mesh.overlay
python line_overlay.py apply original.obj mesh.overlay rebuilt.obj
python -m unittest discover -s .
```

Edit the reconstructed asset in your normal editor, then record it against the same original. Store overlays with Git attribute `-text` so checkout preserves their embedded line endings and byte counts.
