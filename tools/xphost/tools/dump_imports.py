# Dump PE imports/exports of win.xpl with pefile. Usage: python dump_imports.py <xpl> <out.txt>
import sys
import pefile

def main():
    xpl, out = sys.argv[1], sys.argv[2]
    pe = pefile.PE(xpl)
    lines = []
    lines.append("machine=0x%04x" % pe.FILE_HEADER.Machine)
    total = 0
    for entry in pe.DIRECTORY_ENTRY_IMPORT:
        dll = entry.dll.decode("ascii", "replace")
        names = []
        for imp in entry.imports:
            if imp.name:
                names.append(imp.name.decode("ascii", "replace"))
            else:
                names.append("#ordinal%d" % imp.ordinal)
        total += len(names)
        lines.append("DLL %s (%d imports)" % (dll, len(names)))
        for n in sorted(names):
            lines.append("  " + n)
    lines.append("TOTAL %d imports" % total)
    if hasattr(pe, "DIRECTORY_ENTRY_EXPORT"):
        lines.append("EXPORTS")
        for sym in pe.DIRECTORY_ENTRY_EXPORT.symbols:
            lines.append("  " + (sym.name.decode("ascii", "replace") if sym.name else "#%s" % sym.ordinal))
    if hasattr(pe, "DIRECTORY_ENTRY_DELAY_IMPORT"):
        lines.append("DELAY IMPORTS")
        for entry in pe.DIRECTORY_ENTRY_DELAY_IMPORT:
            dll = entry.dll.decode("ascii", "replace")
            names = [i.name.decode("ascii", "replace") if i.name else "#%d" % i.ordinal for i in entry.imports]
            lines.append("DLL %s (%d)" % (dll, len(names)))
            for n in sorted(names):
                lines.append("  " + n)
    text = "\n".join(lines) + "\n"
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(text)

if __name__ == "__main__":
    main()
