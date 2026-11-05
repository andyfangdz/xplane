# Check fake XPLM_64.dll exports cover win.xpl's XPLM imports.
# Usage: python check_exports.py <win.xpl> <XPLM_64.dll> <out.txt>
import sys
import pefile

def imports_of(path, dll_name):
    pe = pefile.PE(path)
    out = set()
    for entry in pe.DIRECTORY_ENTRY_IMPORT:
        if entry.dll.decode("ascii", "replace").lower() == dll_name.lower():
            for imp in entry.imports:
                if imp.name:
                    out.add(imp.name.decode("ascii", "replace"))
    return out

def exports_of(path):
    pe = pefile.PE(path)
    out = set()
    for sym in pe.DIRECTORY_ENTRY_EXPORT.symbols:
        if sym.name:
            out.add(sym.name.decode("ascii", "replace"))
    return out

def main():
    need = imports_of(sys.argv[1], "XPLM_64.dll")
    have = exports_of(sys.argv[2])
    missing = sorted(need - have)
    extra = sorted(have - need)
    lines = ["needed=%d exported=%d missing=%d" % (len(need), len(have), len(missing))]
    lines += ["MISSING " + m for m in missing]
    lines += ["EXTRA " + e for e in extra]
    text = "\n".join(lines) + "\n"
    with open(sys.argv[3], "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(text)
    sys.exit(1 if missing else 0)

if __name__ == "__main__":
    main()
