#!/usr/bin/env python3
"""根拠台帳 (basis.json) の出典を公式文書と照合する。

各検査は `refs` に出典を持つ。出典は "文書ID: 節の見出し" か、
"文書ID: 節の見出し | 本文からの引用" の形の文字列である。文書 ID は
docs/sources.json に載っている IBM の公開文書 (版と sha256 で固定) を指す。

文書そのものはリポジトリに入れない (権利のため)。照合するときに取ってくる。

  tools/basis-refs.py fetch --docs DIR     # PDF を取得し sha256 を確かめて文字にする
  tools/basis-refs.py check --docs DIR     # 見出しと引用が文書にあるかを確かめる
  tools/basis-refs.py report               # スイートごとに出典の付いた検査を数える

check は、見出しが文書のどこかの行にあること、引用がその見出しのあとの本文に
(空白の違いを除いて) そのまま現れることを確かめる。出典が文書と食い違えば非 0 で終わる。
"""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
SUITES = ['zcvs', 'zpvs', 'zhvs']


def sources():
    return json.loads((ROOT / 'docs/sources.json').read_text())


def parse_ref(ref):
    doc, _, rest = ref.partition(':')
    section, _, quote = rest.partition('|')
    return doc.strip(), section.strip(), quote.strip()


def ledger(suite):
    return json.loads((ROOT / suite / 'basis.json').read_text())


def coverage(suite):
    cov = json.loads((ROOT / suite / 'coverage.json').read_text())
    return {p: (v['tests'] if isinstance(v, dict) else v) for p, v in cov.items()}


def checks(suite):
    """(program, check id or None, basis, refs) for every check."""
    led, cov = ledger(suite), coverage(suite)
    for prog, tests in cov.items():
        entry = led.get(prog, {})
        for t in tests or [None]:
            te = entry.get('tests', {}).get(t, {}) if t else {}
            yield (prog, t, te.get('basis', entry.get('basis')),
                   te.get('refs') or entry.get('refs') or [])


def norm(s):
    s = s.replace('­', '').replace('’', "'").replace('‘', "'")
    s = s.replace('“', '"').replace('”', '"').replace('–', '-')
    s = s.replace('—', '-').replace('ﬁ', 'fi').replace('ﬂ', 'fl')
    return re.sub(r'\s+', ' ', s).strip().lower()


def fetch(args):
    out = pathlib.Path(args.docs)
    out.mkdir(parents=True, exist_ok=True)
    for doc, s in sources().items():
        pdf = out / f'{doc}.pdf'
        if not pdf.exists() or hashlib.sha256(pdf.read_bytes()).hexdigest() != s['sha256']:
            print(f'fetch {doc} {s["url"]}')
            with urllib.request.urlopen(s['url']) as r:
                pdf.write_bytes(r.read())
        got = hashlib.sha256(pdf.read_bytes()).hexdigest()
        if got != s['sha256']:
            sys.exit(f'{doc}: sha256 {got} does not match {s["sha256"]}')
        txt = out / f'{doc}.txt'
        if not txt.exists():
            subprocess.run(['pdftotext', '-layout', str(pdf), str(txt)], check=True)


class Doc:
    """見出しは行で引き、引用は見出しのあと (WINDOW 文字以内) の本文で引く。"""
    WINDOW = 60000

    def __init__(self, path):
        text = path.read_text(errors='replace')
        self.heads = {}
        parts, pos = [], 0
        for l in text.splitlines():
            h = norm(l)
            if not h:
                continue
            # 章の見出しは "Chapter 64. INTEGER-PART" の形なので、章番号を外しても引ける
            for k in {h, re.sub(r'^chapter \d+\. ', '', h)}:
                self.heads.setdefault(k, []).append(pos)
            parts.append(h)
            pos += len(h) + 1
        self.body = ' '.join(parts)

    def headings(self, h):
        h = norm(h)
        out = list(self.heads.get(h, []))
        if not out:
            for k, v in self.heads.items():
                if k.startswith(h + ' '):
                    out += v
        return out

    def quote_after(self, at, q):
        q = norm(q)
        for start in at:
            i = self.body.find(q, start, start + self.WINDOW + len(q))
            if i >= 0:
                return True
        return False


def check(args):
    docs, srcs, bad, n = {}, sources(), 0, 0
    seen = set()
    for suite in SUITES:
        for prog, t, basis, refs in checks(suite):
            for ref in refs:
                if ref in seen:
                    continue
                seen.add(ref)
                n += 1
                doc, section, quote = parse_ref(ref)
                if doc not in srcs:
                    print(f'{suite} {prog} {t}: unknown document {doc}')
                    bad += 1
                    continue
                if doc not in docs:
                    docs[doc] = Doc(pathlib.Path(args.docs) / f'{doc}.txt')
                d = docs[doc]
                at = d.headings(section)
                if not at:
                    print(f'{suite} {prog} {t}: heading not in {doc}: {section}')
                    bad += 1
                elif quote and not d.quote_after(at, quote):
                    print(f'{suite} {prog} {t}: quote not under {doc} {section}: {quote}')
                    bad += 1
    print(f'{n} distinct references, {bad} mismatches')
    sys.exit(1 if bad else 0)


def report(args):
    for suite in SUITES:
        total, cited, by = 0, 0, {}
        for prog, t, basis, refs in checks(suite):
            total += 1
            cited += bool(refs)
            by.setdefault(basis, [0, 0])
            by[basis][0] += 1
            by[basis][1] += bool(refs)
        print(f'{suite}: {cited}/{total} checks cited',
              ', '.join(f'{b} {c}/{a}' for b, (a, c) in sorted(by.items())))
        if args.missing:
            for prog, t, basis, refs in checks(suite):
                if not refs:
                    print('  ', prog, t or '')


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='cmd', required=True)
    for name in ['fetch', 'check']:
        p = sub.add_parser(name)
        p.add_argument('--docs', required=True)
    p = sub.add_parser('report')
    p.add_argument('--missing', action='store_true')
    args = ap.parse_args()
    {'fetch': fetch, 'check': check, 'report': report}[args.cmd](args)


if __name__ == '__main__':
    main()
