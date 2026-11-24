"""Record and replay text edits against a separately supplied X-Plane asset."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tempfile


def lines(data):
    # Git delimits lines with LF even when a record contains a bare CR.
    records = data.split(b"\n")
    return [row + b"\n" for row in records[:-1]] + ([records[-1]] if records[-1] else [])


def record(original, edited):
    before, after = original.read_bytes(), edited.read_bytes()
    if b"\0" in before or b"\0" in after:
        raise ValueError("Expected text input")
    source, target = lines(before), lines(after)
    diff = subprocess.run(["git", "-c", "core.autocrlf=false", "-c", "diff.algorithm=histogram", "diff", "--no-index",
                           "--no-color", "--no-textconv", "--no-ext-diff", "--text", "--ignore-cr-at-eol", "--unified=0",
                           "--", str(original.resolve()), str(edited.resolve())],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if diff.returncode not in (0, 1):
        raise RuntimeError(diff.stderr.decode(errors="replace"))
    chunks = [b"line-overlay-v1 " + hashlib.sha256(before).hexdigest().encode() + b"\n"]
    i = j = 0

    def copy(stop):
        nonlocal i, j
        count = stop - i
        if count < 0:
            raise ValueError("Diff does not describe the supplied bytes")
        run, previous = i, None
        for index in range(i, stop):
            line, wanted = source[index], target[j + index - i]
            if line == wanted:
                mode = "copy"
            elif line.endswith(b"\r\n") and line[:-2] + b"\n" == wanted:
                mode = "copy-lf"
            elif line.endswith(b"\n") and line[:-1] + b"\r\n" == wanted:
                mode = "copy-crlf"
            else:
                # Git can match the last line across a missing final newline.
                if line.rstrip(b"\r\n") != wanted.rstrip(b"\r\n"):
                    raise ValueError("Diff does not describe the supplied bytes")
                if previous:
                    chunks.append(f"{previous} {run + 1} {index - run}\n".encode())
                chunks.extend([f"add {len(wanted)}\n".encode(), wanted, b"\n"])
                previous = None
                continue
            if mode != previous:
                if previous:
                    chunks.append(f"{previous} {run + 1} {index - run}\n".encode())
                run, previous = index, mode
        if previous:
            chunks.append(f"{previous} {run + 1} {stop - run}\n".encode())
        i, j = stop, j + count

    for match in re.finditer(rb"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", diff.stdout, re.M):
        old, removed, new, added = (int(v) if v is not None else 1 for v in match.groups())
        copy(old - bool(removed))
        if j != new - bool(added):
            raise ValueError("Inconsistent hunk offsets")
        if added:
            data = b"".join(target[j:j + added])
            chunks.extend([f"add {len(data)}\n".encode(), data, b"\n"])
        i += removed
        j += added
    copy(len(source))
    if j != len(target):
        raise ValueError("Incomplete diff")
    return b"".join(chunks)


def apply(original, overlay):
    data = original.read_bytes()
    header = b"line-overlay-v1 " + hashlib.sha256(data).hexdigest().encode() + b"\n"
    records = lines(data)
    with overlay.open("rb") as stream:
        if stream.readline() != header:
            raise ValueError(f"Base file does not match {overlay.name}")
        chunks = []
        while op := stream.readline():
            if match := re.fullmatch(rb"(copy|copy-lf|copy-crlf) (\d+) (\d+)\n", op):
                mode, first, length = match.groups()
                start, count = int(first), int(length)
                if start < 1 or count < 1 or start + count - 1 > len(records):
                    raise ValueError("Copy outside base file")
                region = records[start - 1:start - 1 + count]
                if mode == b"copy-lf":
                    chunks.extend(row.removesuffix(b"\r\n") + b"\n" for row in region)
                elif mode == b"copy-crlf":
                    chunks.extend(row.removesuffix(b"\n") + b"\r\n" for row in region)
                else:
                    chunks.extend(region)
            elif match := re.fullmatch(rb"add (\d+)\n", op):
                count = int(match[1])
                if count > overlay.stat().st_size - stream.tell():
                    raise ValueError("Truncated addition")
                literal = stream.read(count)
                if stream.read(1) != b"\n":
                    raise ValueError("Missing addition terminator")
                chunks.append(literal)
            else:
                raise ValueError(f"Unknown overlay record: {op[:80]!r}")
    return b"".join(chunks)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["record", "apply"])
    parser.add_argument("original", type=Path)
    parser.add_argument("input", type=Path, help="edited text for record, overlay for apply")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.resolve() in (args.original.resolve(), args.input.resolve()):
        parser.error("Choose a separate output path")
    result = (record if args.operation == "record" else apply)(args.original, args.input)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=args.output.parent, delete=False) as stream:
        temp = Path(stream.name)
    try:
        temp.write_bytes(result)
        os.replace(temp, args.output)
    finally:
        temp.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
