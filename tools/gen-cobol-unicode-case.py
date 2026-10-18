"""Generate COBOL simple case mappings from an official UnicodeData.txt.

Usage: python tools/gen-cobol-unicode-case.py PATH/UnicodeData.txt
Input: https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt
Run cargo fmt --all afterwards. The associated Unicode license is kept
beside the generated Rust file.
"""

import pathlib
import sys


def ranges(rows, column):
    result = []
    for row in rows:
        if not row[column]:
            continue
        value = int(row[0], 16)
        delta = int(row[column], 16) - value
        if result and result[-1][1] + 1 == value and result[-1][2] == delta:
            start, _, _ = result[-1]
            result[-1] = (start, value, delta)
        else:
            result.append((value, value, delta))
    return result


def main():
    rows = [line.split(";") for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
    target = pathlib.Path(__file__).resolve().parents[1] / "crates/zeus-cobol/src/vm/unicode_case.rs"
    # Preserve the documented runtime lookup while replacing both generated tables.
    prefix = target.read_text(encoding="utf-8").split("const UPPER:", 1)[0]
    output = prefix
    for name, column in [("UPPER", 12), ("LOWER", 13)]:
        output += f"const {name}: &[(u32, u32, i32)] = &[\n"
        for start, end, delta in ranges(rows, column):
            output += f"    (0x{start:X}, 0x{end:X}, {delta}),\n"
        output += "];\n"
    target.write_text(output, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
