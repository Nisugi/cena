"""A small Ruby lexer for .lic census work. Not a parser.

lex(src) -> Lexed with:
  code     : source with comments removed, every string/regex/heredoc/%-literal
             replaced by a placeholder  __S<n>__  (strings) or __R<n>__ (regexes).
             Newlines inside removed literals are kept, so line numbers survive.
  strings  : content of each string placeholder (interpolation kept as raw '#{...}')
  regexes  : content of each regex placeholder
  interp   : lexed code of every #{...} interpolation (strings inside it are placeholders too)
  backticks: number of `...` and %x literals (shell commands)
  errors   : list of problems (unterminated literal etc.)

Handled: =begin/=end, __END__, # comments, '..', "..", `..`, :"..", %w %i %q %Q %r %x %s %(..),
/regex/ (division-vs-regex by previous token and known local variables), heredocs <<~ <<- <<ID,
?c character literals, $' $" $` special globals.
"""
import re

KW_REGEX_OK = {
    'if', 'unless', 'when', 'and', 'or', 'not', 'return', 'while', 'until', 'elsif', 'then',
    'do', 'case', 'yield', 'else', 'in', 'begin', 'rescue', 'ensure', 'break', 'next', 'raise',
    'fail', 'puts', 'print', 'p', 'loop', 'lambda', 'proc',
}
DELIMS = {'(': ')', '[': ']', '{': '}', '<': '>'}
IDENT_START = re.compile(r'[A-Za-z_\x80-￿]')
IDENT = re.compile(r'[A-Za-z_\x80-￿][A-Za-z0-9_\x80-￿]*')
NUM = re.compile(r'0[xX][0-9a-fA-F_]+|0[bB][01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?')
HEREDOC = re.compile(r'<<([~-]?)(["\'`]?)([A-Za-z_][A-Za-z0-9_]*)\2')
ASSIGNED = re.compile(r'(?<![.\w@$:])([a-z_][A-Za-z0-9_]*)\s*(?:=(?![=~>])|\+=|-=|\|\|=)')
BLOCKPARAM = re.compile(r'\|([^|\n]*)\|')


class Lexed:
    def __init__(self):
        self.code = ''
        self.strings = []
        self.regexes = []
        self.interp = []
        self.backticks = 0
        self.errors = []


def prepass(src):
    """Blank =begin/=end blocks and everything after __END__, keeping line count."""
    lines = src.split('\n')
    out = []
    in_doc = False
    ended = False
    for ln in lines:
        if ended:
            out.append('')
            continue
        if in_doc:
            if ln.startswith('=end'):
                in_doc = False
            out.append('')
            continue
        if ln.startswith('=begin'):
            in_doc = True
            out.append('')
            continue
        if ln.rstrip('\r') == '__END__':
            ended = True
            out.append('')
            continue
        out.append(ln)
    return '\n'.join(out)


class _L:
    def __init__(self, s, res, locals_):
        self.s = s
        self.n = len(s)
        self.i = 0
        self.res = res
        self.locals = locals_

    def err(self, msg):
        line = self.s.count('\n', 0, self.i) + 1
        self.res.errors.append(f'{msg} at line {line}')

    # ---- literal scanners -------------------------------------------------
    def add_string(self, content):
        self.res.strings.append(content)
        return f'__S{len(self.res.strings) - 1}__'

    def add_regex(self, content):
        self.res.regexes.append(content)
        return f'__R{len(self.res.regexes) - 1}__'

    def scan_interp(self):
        """self.i is just after '#{'. Scan code to the matching '}', return raw text."""
        start = self.i
        out = []
        self.scan_code(out, stop_brace=True)
        self.res.interp.append(''.join(out))
        return '#{' + self.s[start:self.i - 1] + '}' if self.i <= self.n else '#{' + self.s[start:]

    def scan_delimited(self, close, open_=None, interp=True, regex=False):
        """self.i just after the opening delimiter. Returns (content, newline_count)."""
        s, n = self.s, self.n
        buf = []
        depth = 0
        nl = 0
        while self.i < n:
            c = s[self.i]
            if c == '\\':
                buf.append(s[self.i:self.i + 2])
                if self.i + 1 < n and s[self.i + 1] == '\n':
                    nl += 1
                self.i += 2
                continue
            if c == '\n':
                nl += 1
            if interp and c == '#' and self.i + 1 < n and s[self.i + 1] == '{':
                self.i += 2
                before = self.s.count('\n', 0, self.i)
                buf.append(self.scan_interp())
                nl += self.s.count('\n', 0, self.i) - before
                continue
            if open_ is not None and c == open_:
                depth += 1
            elif c == close:
                if depth == 0:
                    self.i += 1
                    return ''.join(buf), nl
                depth -= 1
            buf.append(c)
            self.i += 1
        self.err(f'unterminated literal ({close})')
        return ''.join(buf), nl

    # ---- main scanner ------------------------------------------------------
    def scan_code(self, out, stop_brace=False):
        s, n = self.s, self.n
        prev = 'start'      # start op open kw ident val
        prev_ident = ''
        space = False       # whitespace just before current char
        brace = 0
        pending = []        # heredocs: (id, mod, interp, index)
        while self.i < n:
            c = s[self.i]
            if c == '\n':
                out.append('\n')
                self.i += 1
                if pending:
                    for hid, mod, interp, idx in pending:
                        body = []
                        while self.i < n:
                            j = s.find('\n', self.i)
                            line = s[self.i:] if j < 0 else s[self.i:j]
                            self.i = n if j < 0 else j + 1
                            out.append('\n')
                            test = line.strip() if mod else line.rstrip('\r')
                            if test == hid:
                                break
                            body.append(line)
                        else:
                            self.err(f'unterminated heredoc {hid}')
                        text = '\n'.join(body)
                        if interp:
                            for m in re.finditer(r'#\{', text):
                                # crude brace match for heredoc interpolation
                                k, d = m.end(), 1
                                while k < len(text) and d:
                                    d += {'{': 1, '}': -1}.get(text[k], 0)
                                    k += 1
                                sub = lex(text[m.end():k - 1], prepassed=True)
                                # the sub-lexer's placeholders index its own tables: rename them
                                self.res.interp.append(re.sub(r'__([SR])(\d+)__', r'__H\1\2__', sub.code))
                                self.res.interp.extend(re.sub(r'__([SR])(\d+)__', r'__H\1\2__', x) for x in sub.interp)
                        self.res.strings[idx] = text
                    pending = []
                prev, space = 'start', False
                continue
            if c in ' \t\r\f':
                out.append(c)
                self.i += 1
                space = True
                continue
            if c == '\\' and self.i + 1 < n and s[self.i + 1] in '\r\n':
                out.append('\\')
                self.i += 1
                space = True
                continue
            if c == '#':
                j = s.find('\n', self.i)
                self.i = n if j < 0 else j
                continue
            was_space, space = space, False
            if stop_brace:
                if c == '{':
                    brace += 1
                elif c == '}':
                    if brace == 0:
                        self.i += 1
                        return
                    brace -= 1
            # strings
            if c == "'" or c == '"' or c == '`':
                self.i += 1
                content, nl = self.scan_delimited(c, interp=(c != "'"))
                if c == "'":
                    content = content.replace("\\'", "'").replace('\\\\', '\\')
                if c == '`':
                    self.res.backticks += 1
                out.append(self.add_string(content) + '\n' * nl)
                prev = 'val'
                continue
            if c == ':' and self.i + 1 < n and s[self.i + 1] in '"\'' and not (self.i > 0 and s[self.i - 1] == ':'):
                q = s[self.i + 1]
                self.i += 2
                content, nl = self.scan_delimited(q, interp=(q == '"'))
                out.append(':' + self.add_string(content) + '\n' * nl)
                prev = 'val'
                continue
            if c == '%' and self.i + 1 < n:
                m = re.match(r'%([wWiIqQrxs]?)([^\w\s])', s[self.i:self.i + 3])
                if m:
                    kind, d = m.group(1), m.group(2)
                    ok_prev = prev in ('start', 'op', 'open', 'kw') or (prev == 'ident' and was_space and prev_ident not in self.locals)
                    if kind == '' and d not in '([{<|!/^':
                        ok = False
                    elif kind == '' and d == '=':
                        ok = False
                    else:
                        ok = ok_prev
                    if ok:
                        self.i += len(m.group(0))
                        close = DELIMS.get(d, d)
                        open_ = d if d in DELIMS else None
                        interp = kind in ('', 'W', 'I', 'Q', 'r', 'x')
                        content, nl = self.scan_delimited(close, open_, interp=interp, regex=(kind == 'r'))
                        if kind == 'r':
                            while self.i < n and s[self.i] in 'imxounse':
                                self.i += 1
                            out.append(self.add_regex(content) + '\n' * nl)
                        else:
                            if kind == 'x':
                                self.res.backticks += 1
                            out.append(self.add_string(content) + '\n' * nl)
                        prev = 'val'
                        continue
            if c == '/':
                if prev in ('start', 'op', 'open', 'kw'):
                    is_re = True
                elif prev == 'ident':
                    nxt = s[self.i + 1] if self.i + 1 < n else ' '
                    is_re = was_space and nxt not in ' =\t' and prev_ident not in self.locals
                else:
                    is_re = False
                if is_re:
                    self.i += 1
                    content, nl = self.scan_delimited('/', interp=True, regex=True)
                    while self.i < n and s[self.i] in 'imxounse':
                        self.i += 1
                    out.append(self.add_regex(content) + '\n' * nl)
                    prev = 'val'
                    continue
                out.append(c)
                self.i += 1
                prev = 'op'
                continue
            if c == '<' and s.startswith('<<', self.i):
                m = HEREDOC.match(s, self.i)
                if m:
                    mod, q, hid = m.group(1), m.group(2), m.group(3)
                    ok = bool(mod) or bool(q) or (
                        hid.isupper() and (prev in ('start', 'op', 'open', 'kw') or (prev == 'ident' and was_space and prev_ident not in self.locals)))
                    if mod and not q and prev in ('val',) and not was_space:
                        ok = False
                    if ok:
                        self.res.strings.append('')
                        idx = len(self.res.strings) - 1
                        pending.append((hid, mod, q != "'", idx))
                        out.append(f'__S{idx}__')
                        self.i = m.end()
                        prev = 'val'
                        continue
                out.append('<<')
                self.i += 2
                prev = 'op'
                continue
            if c == '?' and prev in ('start', 'op', 'open', 'kw') and self.i + 1 < n and s[self.i + 1] not in ' \t\r\n':
                if s[self.i + 1] == '\\':
                    self.i += 3
                else:
                    self.i += 2
                out.append('__C__')
                prev = 'val'
                continue
            if c == '$':
                m = re.match(r"\$(?:[A-Za-z_][A-Za-z0-9_]*|-[A-Za-z0-9]|\d+|[~*$?!@/\\;,.=:<>\"`'&+_0])", s[self.i:self.i + 64])
                tok = m.group(0) if m else '$'
                out.append(tok)
                self.i += len(tok)
                prev = 'val'
                continue
            if c == '@':
                m = re.match(r'@@?[A-Za-z_][A-Za-z0-9_]*', s[self.i:self.i + 128])
                tok = m.group(0) if m else '@'
                out.append(tok)
                self.i += len(tok)
                prev = 'val'
                continue
            if IDENT_START.match(c):
                m = IDENT.match(s, self.i)
                tok = m.group(0)
                j = m.end()
                if j < n and s[j] in '?!' and not (j + 1 < n and s[j + 1] == '=' and not (j + 2 < n and s[j + 2] == '=')):
                    # method?  /  method!  (but not a != b)
                    if not (s[j] == '?' and j + 1 < n and s[j + 1] == ':' and not (j + 2 < n and s[j + 2] == ':')):
                        tok += s[j]
                        j += 1
                out.append(tok)
                self.i = j
                if tok in KW_REGEX_OK:
                    prev = 'kw'
                elif tok in ('end', 'self', 'true', 'false', 'nil'):
                    prev = 'val'
                else:
                    prev = 'ident'
                prev_ident = tok
                continue
            if c.isdigit():
                m = NUM.match(s, self.i)
                out.append(m.group(0))
                self.i = m.end()
                prev = 'val'
                continue
            out.append(c)
            self.i += 1
            if c in ')]}':
                prev = 'val'
            elif c in '([{,|;':
                prev = 'open'
            else:
                prev = 'op'
        if stop_brace:
            self.err('unterminated interpolation')


def lex(src, prepassed=False):
    res = Lexed()
    text = src if prepassed else prepass(src)
    locals_ = set(ASSIGNED.findall(text))
    for bp in BLOCKPARAM.findall(text):
        for name in re.findall(r'[a-z_][A-Za-z0-9_]*', bp):
            locals_.add(name)
    lx = _L(text, res, locals_)
    out = []
    lx.scan_code(out)
    res.code = ''.join(out)
    return res
