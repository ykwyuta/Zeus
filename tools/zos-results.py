#!/usr/bin/env python3
"""Record a z/OS run of a validation suite and compare it with the basis ledger.

The suites (zcvs, zpvs, zhvs) are exported with `zeus-ccvs export-jcl` and run on
z/OS. This script takes what comes back and turns it into
`<suite>/zos-results/<date>.json`, then reads that file against `basis.json`:

  record   Read the REPORT members (one file per program) and, optionally, a
           step file with the compile return code and run completion of each
           program. Every check ID of coverage.json gets PASS, FAIL or MISSING.
  compare  Group the recorded checks by basis and say what each failure means:
           spec FAIL -> the check is wrong (re-read the manual), zos FAIL -> Zeus
           differs from z/OS (take the z/OS value, fix the check and Zeus),
           zos PASS -> the Zeus value is confirmed on z/OS.

A report member may be the raw dataset (fixed 120-byte EBCDIC records) or a
text download (one line per record). A report line whose first word is PASS
or FAIL* is a check; the next word is its ID (all three suites write this).

The step file is plain text, one program per line:

  PROGRAM  COMPILE-RC  COMPLETION
  ZA124A   0           U4038
  HS105A   0           S0C7
  ZE101N   12          -

COMPLETION is the run step's return code (a number), an abend code (Sxxx or
Uxxxx), or '-' when the step did not run. Lines starting with '#' are ignored.

`record` also reads a Zeus work directory (`zeus-ccvs run --work W`, reports in
W/<toolchain>/<module>/<PROGRAM>.log), so the same file can be made from Zeus and
compared: a Zeus run must give no spec or zos failure.

usage:
  tools/zos-results.py record --suite zcvs --reports DIR [--steps FILE]
      [--env KEY=VALUE]... [--date YYYY-MM-DD] [--out FILE]
  tools/zos-results.py compare --suite zcvs --results FILE
"""
import argparse
import datetime
import json
import os
import sys
import tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def load_suite(suite):
    base = os.path.join(ROOT, suite)
    with open(os.path.join(base, 'coverage.json'), encoding='utf-8') as f:
        coverage = json.load(f)
    with open(os.path.join(base, 'basis.json'), encoding='utf-8') as f:
        basis = json.load(f)
    with open(os.path.join(base, 'manifest.toml'), 'rb') as f:
        manifest = tomllib.load(f).get('programs', {})
    return coverage, basis, manifest


def decode(data):
    """Text of a report member: EBCDIC fixed records or an ASCII download."""
    try:
        text = data.decode('ascii')
        if 'PASS' in text or 'FAIL' in text or not data:
            return text.splitlines()
    except UnicodeDecodeError:
        pass
    if len(data) % 120 == 0:
        return [data[i:i + 120].decode('cp037').rstrip() for i in range(0, len(data), 120)]
    # a text download in an EBCDIC code page with line ends
    return data.decode('cp037').splitlines()


def checks(lines):
    """[(id, 'PASS'|'FAIL')] in report order."""
    out = []
    for line in lines:
        words = line.split()
        if len(words) >= 2 and words[0] in ('PASS', 'FAIL*'):
            out.append((words[1], 'PASS' if words[0] == 'PASS' else 'FAIL'))
    return out


def find_reports(directory, programs):
    """PROGRAM -> path. Accepts PROGRAM, PROGRAM.<ext> and <module>/PROGRAM.log."""
    found = {}
    for dirpath, _, files in os.walk(directory):
        for name in files:
            stem = name.split('.')[0].upper()
            if stem not in programs:
                continue
            ext = name[len(stem):].lower()
            if ext in ('', '.txt', '.bin', '.rpt', '.report', '.log'):
                # a Zeus work directory also holds .out (stdout) and .compile.log
                found.setdefault(stem, os.path.join(dirpath, name))
    return found


def read_steps(path):
    steps = {}
    if not path:
        return steps
    with open(path, encoding='utf-8') as f:
        for line in f:
            words = line.split()
            if not words or words[0].startswith('#') or words[0] == 'PROGRAM':
                continue
            if len(words) != 3:
                sys.exit(f'{path}: expected PROGRAM COMPILE-RC COMPLETION: {line.rstrip()}')
            steps[words[0].upper()] = {'compile_rc': None if words[1] == '-' else int(words[1]),
                                       'completion': None if words[2] == '-' else words[2].upper()}
    return steps


def record(args):
    coverage, _, manifest = load_suite(args.suite)
    reports = find_reports(args.reports, set(coverage))
    steps = read_steps(args.steps)
    programs = {}
    for program, spec in sorted(coverage.items()):
        entry = {}
        if program in steps:
            entry.update(steps[program])
        if spec['expect'] == 'run':
            seen = []
            if program in reports:
                with open(reports[program], 'rb') as f:
                    seen = checks(decode(f.read()))
            got = dict(seen)
            entry['tests'] = {t: got.get(t, 'MISSING') for t in spec['tests']}
            extra = [t for t, _ in seen if t not in spec['tests']]
            if extra:
                entry['unexpected'] = extra
            order = [t for t, _ in seen if t in spec['tests']]
            if order != [t for t in spec['tests'] if t in got]:
                entry['order_changed'] = True
            if program not in reports:
                entry['report'] = 'missing'
        programs[program] = entry
    env = dict(e.split('=', 1) for e in args.env)
    result = {'suite': args.suite, 'date': args.date, 'environment': env, 'programs': programs}
    out = args.out or os.path.join(ROOT, args.suite, 'zos-results', f'{args.date}.json')
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, 'w', encoding='utf-8') as f:
        json.dump(result, f, indent=2, ensure_ascii=False)
        f.write('\n')
    print(f'{out}: {len(programs)} programs, {len(reports)} reports')


def basis_of(basis, program, test):
    entry = basis.get(program, {})
    override = entry.get('tests', {}).get(test)
    if override:
        return override['basis'], override.get('note', '')
    return entry.get('basis', 'spec'), entry.get('note', '')


def completion_ok(expected_rc, completion):
    """Whether a run completion matches the manifest. A manifest exit code says the
    step must not end normally with RC 0; z/OS gives an abend code there."""
    if completion is None:
        return None
    if expected_rc is None:
        return completion in ('0', '00', '0000')
    return completion not in ('0', '00', '0000')


def compare(args):
    coverage, basis, manifest = load_suite(args.suite)
    with open(args.results, encoding='utf-8') as f:
        result = json.load(f)
    if result.get('suite') != args.suite:
        sys.exit(f'{args.results} is for suite {result.get("suite")}, not {args.suite}')
    counts = {}
    findings = {'spec-FAIL': [], 'zos-FAIL': [], 'zeus-FAIL': [], 'MISSING': [], 'STEP': []}
    confirmed = []
    for program, spec in sorted(coverage.items()):
        entry = result['programs'].get(program)
        if entry is None:
            findings['MISSING'].append((program, '-', 'program not in the results'))
            continue
        cfg = manifest.get(program, {})
        if spec['expect'] == 'compile-error':
            rc = entry.get('compile_rc')
            if rc is not None and rc < 8:
                findings['STEP'].append((program, '-', f'compile RC {rc}; a rejection (RC>=8) is expected'))
            continue
        if spec['expect'] == 'compile-only':
            rc = entry.get('compile_rc')
            if rc is not None and rc > 4:
                findings['STEP'].append((program, '-', f'compile RC {rc}; acceptance (RC<=4) is expected'))
            continue
        if entry.get('report') == 'missing':
            n = len(spec['tests'])
            findings['MISSING'].append((program, '-', f'no report ({n} checks)'))
            for test in spec['tests']:
                b, _ = basis_of(basis, program, test)
                counts.setdefault(b, {'PASS': 0, 'FAIL': 0, 'MISSING': 0})['MISSING'] += 1
            continue
        ok = completion_ok(cfg.get('expect_exit_code'), entry.get('completion'))
        if ok is False:
            want = 'an abnormal end' if cfg.get('expect_exit_code') is not None else 'RC 0'
            findings['STEP'].append((program, '-', f'completion {entry["completion"]}; {want} is expected'))
        for test, status in entry.get('tests', {}).items():
            b, note = basis_of(basis, program, test)
            counts.setdefault(b, {'PASS': 0, 'FAIL': 0, 'MISSING': 0})[status] += 1
            if status == 'FAIL':
                findings[f'{b}-FAIL'].append((program, test, note))
            elif status == 'MISSING':
                findings['MISSING'].append((program, test, b))
            elif b == 'zos':
                confirmed.append((program, test))
    print(f'{args.suite} {result.get("date")} {result.get("environment", {})}')
    for b in ('spec', 'zos', 'zeus'):
        c = counts.get(b, {'PASS': 0, 'FAIL': 0, 'MISSING': 0})
        print(f'  {b:5} PASS {c["PASS"]:5}  FAIL {c["FAIL"]:4}  MISSING {c["MISSING"]:4}')
    meaning = {
        'spec-FAIL': 'spec FAIL: the check disagrees with the manual as read; re-read it and fix the check',
        'zos-FAIL': 'zos FAIL: Zeus differs from z/OS; take the z/OS value, fix the check and Zeus',
        'zeus-FAIL': 'zeus FAIL: a Zeus-only selection; adjust how the check is run on z/OS',
        'MISSING': 'MISSING: no result for these checks (report absent or the run ended early)',
        'STEP': 'STEP: compile or run completion differs from the manifest',
    }
    for key, items in findings.items():
        if items:
            print(f'\n{meaning[key]}')
            for program, test, note in items:
                print(f'  {program} {test}  {note}')
    if confirmed:
        print(f'\nzos PASS: {len(confirmed)} checks confirmed on z/OS; move their basis to spec '
              'with the run date in the note')
    bad = sum(len(v) for v in findings.values())
    return 1 if bad else 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='cmd', required=True)
    r = sub.add_parser('record')
    r.add_argument('--suite', required=True, choices=['zcvs', 'zpvs', 'zhvs'])
    r.add_argument('--reports', required=True)
    r.add_argument('--steps')
    r.add_argument('--env', action='append', default=[], help='KEY=VALUE, e.g. compiler="Enterprise COBOL 6.4"')
    r.add_argument('--date', default=datetime.date.today().isoformat())
    r.add_argument('--out')
    c = sub.add_parser('compare')
    c.add_argument('--suite', required=True, choices=['zcvs', 'zpvs', 'zhvs'])
    c.add_argument('--results', required=True)
    args = p.parse_args()
    if args.cmd == 'record':
        record(args)
        return 0
    return compare(args)


if __name__ == '__main__':
    sys.exit(main())
