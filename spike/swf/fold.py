#!/usr/bin/env python3
"""Folds `swfspike dis` output (AVM1 stack ops) into pseudo-ActionScript.

    swfspike/target/release/swfspike dis data/interface/inventorymenu.swf > out/inv.dis
    python3 fold.py out/inv.dis > out/inv.as

Pushes, member reads and calls are folded into expressions, printed when a
statement ends them (a set, a pop, a return). Branches stay as
`@offset if (cond) goto @target`; values left on the stack at a jump are
printed as `/*left*/`. In methods, `r1` is `this` and `r2` the superclass
(`r2.InitExtensions()` is `super.InitExtensions()`). Good enough to read the
classes, not to recompile them.
"""
import re, sys

TOK = re.compile(r'\s*("(?:[^"\\]|\\.)*"|r\d+|Int\(-?\d+\)|Double\([^)]*\)|Bool\(\w+\)|Undefined|Null|[^,]+)\s*,?')
BIN = {'Add2':'+','Subtract':'-','Multiply':'*','Divide':'/','Equals2':'==','StrictEquals':'===',
       'Less2':'<','Greater':'>','BitAnd':'&','BitOr':'|','BitRShift':'>>','BitLShift':'<<',
       'BitURShift':'>>>','BitXor':'^','InstanceOf':'instanceof','Modulo':'%'}

def pushvals(s):
    out = []
    for m in TOK.finditer(s):
        t = m.group(1).strip()
        if not t: continue
        if t.startswith('Int(') or t.startswith('Double('): t = t[t.index('(')+1:-1]
        elif t.startswith('Bool('): t = t[5:-1]
        elif t == 'Undefined': t = 'undefined'
        elif t == 'Null': t = 'null'
        out.append(t)
    return out

class Ctx:
    def __init__(self, head, indent):
        self.head, self.indent, self.stack, self.out = head, indent, [], []
    def pop(self):
        return self.stack.pop() if self.stack else '?'
    def popn(self, n):
        try: n = int(float(n))
        except: return ['?args']
        return [self.pop() for _ in range(n)]
    def emit(self, s): self.out.append(s)

def main(path):
    lines = open(path, encoding='utf-8', errors='replace').read().split('\n')
    ctxs = [Ctx(None, -1)]
    for line in lines:
        if line.startswith('=='):
            for l in ctxs[0].out: print(l)
            print(line); ctxs = [Ctx(None, -1)]; continue
        m = re.match(r'^(\s*)(\d+) (.*)$', line)
        c = ctxs[-1]
        if not m:
            if line.strip() == '}' and len(ctxs) > 1:
                done = ctxs.pop(); c = ctxs[-1]
                for v in done.stack: done.emit('/*left*/ ' + v)
                body = '\n'.join('    ' + l for l in done.out)
                c.stack.append(f'{done.head} {{\n{body}\n}}')
            continue
        off, rest = int(m.group(2)), m.group(3)
        op = re.match(r'(\w+)', rest).group(1)
        arg = rest[len(op):].strip()
        if op == 'function':
            ctxs.append(Ctx(rest.rstrip(' {'), len(m.group(1))))
            continue
        s = c
        if op == 'ConstantPool': continue
        if op == 'Push': s.stack += pushvals(arg)
        elif op == 'GetVariable': s.stack.append(s.pop().strip('"'))
        elif op == 'GetMember':
            k = s.pop(); o = s.pop()
            s.stack.append(f'{o}.{k[1:-1]}' if k.startswith('"') and re.match(r'^"\w+"$', k) else f'{o}[{k}]')
        elif op == 'SetMember':
            v = s.pop(); k = s.pop(); o = s.pop()
            s.emit(f'{o}.{k[1:-1]} = {v}' if re.match(r'^"\w+"$', k) else f'{o}[{k}] = {v}')
        elif op in ('SetVariable', 'DefineLocal'):
            v = s.pop(); k = s.pop(); s.emit(f'{"var " if op=="DefineLocal" else ""}{k.strip(chr(34))} = {v}')
        elif op == 'CallMethod':
            name = s.pop(); obj = s.pop(); a = s.popn(s.pop())
            f = f'{obj}.{name[1:-1]}' if re.match(r'^"\w+"$', name) else (obj if name == 'undefined' else f'{obj}[{name}]')
            s.stack.append(f'{f}({", ".join(a)})')
        elif op == 'CallFunction':
            name = s.pop().strip('"'); a = s.popn(s.pop()); s.stack.append(f'{name}({", ".join(a)})')
        elif op == 'NewObject':
            name = s.pop().strip('"'); a = s.popn(s.pop()); s.stack.append(f'new {name}({", ".join(a)})')
        elif op == 'NewMethod':
            name = s.pop(); obj = s.pop(); a = s.popn(s.pop()); s.stack.append(f'new {obj}.{name.strip(chr(34))}({", ".join(a)})')
        elif op == 'InitObject':
            n = int(float(s.pop())); kv = []
            for _ in range(n):
                v = s.pop(); k = s.pop(); kv.append(f'{k.strip(chr(34))}: {v}')
            s.stack.append('{' + ', '.join(kv) + '}')
        elif op == 'InitArray':
            s.stack.append('[' + ', '.join(s.popn(s.pop())) + ']')
        elif op == 'Pop':
            v = s.pop()
            if v != '?': s.emit(v)
        elif op == 'StoreRegister':
            r = re.search(r'register: (\d+)', arg).group(1)
            v = s.pop(); s.emit(f'r{r} = {v}'); s.stack.append(f'r{r}')
        elif op in BIN:
            b = s.pop(); a = s.pop(); s.stack.append(f'({a} {BIN[op]} {b})')
        elif op == 'Not': s.stack.append(f'!{s.pop()}')
        elif op == 'PushDuplicate':
            v = s.pop(); s.stack += [v, v]
        elif op in ('Increment', 'Decrement'):
            s.stack.append(f'({s.pop()} {"+" if op=="Increment" else "-"} 1)')
        elif op in ('TypeOf', 'ToString', 'ToNumber', 'TargetPath'):
            s.stack.append(f'{op}({s.pop()})')
        elif op == 'CastOp':
            o = s.pop(); t = s.pop(); s.stack.append(f'({t})({o})')
        elif op == 'Delete':
            k = s.pop(); o = s.pop(); s.stack.append(f'delete {o}[{k}]')
        elif op == 'Enumerate2':
            s.emit(f'for-in {s.pop()}')
        elif op == 'Trace': s.emit(f'trace({s.pop()})')
        elif op == 'Return': s.emit(f'return {s.pop()}')
        elif op == 'Extends':
            sup = s.pop(); sub = s.pop(); s.emit(f'{sub} extends {sup}')
        elif op == 'If':
            t = off + 5 + int(re.search(r'offset: (-?\d+)', arg).group(1))
            s.emit(f'@{off} if ({s.pop()}) goto @{t}')
        elif op == 'Jump':
            t = off + 5 + int(re.search(r'offset: (-?\d+)', arg).group(1))
            for v in s.stack: s.emit('/*left*/ ' + v)
            s.stack = []
            s.emit(f'@{off} goto @{t}')
        else:
            s.emit(f'@{off} {rest}')
    for l in ctxs[0].out: print(l)

main(sys.argv[1])
