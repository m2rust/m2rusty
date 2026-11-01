import sys
sys.path.insert(0, GEN_DIR)

import math, time, dbg
import gen_math, gen_time, gen_dbg, gen_pack

checks = [0]
fails = []


def call(f, args):
    try:
        return ("ok", repr(f(*args)))
    except Exception, e:
        return ("exc", e.__class__.__name__, str(e))


def same(label, hand, gen, args, kind_only=False):
    checks[0] += 1
    a = call(hand, args)
    b = call(gen, args)
    if kind_only and a[0] == "exc" and b[0] == "exc":
        a, b = a[:2], b[:2]
    if a != b:
        fails.append("%s%r: hand=%r generated=%r" % (label, args, a, b))


NUMS = [0, 1, -1, 0.5, -0.5, 2, 10, 1e10, -3.75, 1e-9, 100, float("inf"), float("-inf")]
for name in ["sin", "cos", "tan", "asin", "acos", "atan", "sqrt", "ceil", "floor", "fabs", "exp",
             "log10", "degrees", "radians", "modf"]:
    for x in NUMS + [True, 3L, "x", None, (), [1]]:
        same("math." + name, getattr(math, name), getattr(gen_math, name), (x,))
    same("math." + name, getattr(math, name), getattr(gen_math, name), ())
    same("math." + name, getattr(math, name), getattr(gen_math, name), (1, 2))

for name in ["atan2", "hypot", "pow", "fmod"]:
    for x in [0, 1, -2.5, 3]:
        for y in [0, 1, -2.5, 4, "s"]:
            same("math." + name, getattr(math, name), getattr(gen_math, name), (x, y))
    same("math." + name, getattr(math, name), getattr(gen_math, name), (1,))

for x in [1, 2.5, 100, 0, -1, "a"]:
    same("math.log", math.log, gen_math.log, (x,))
    for base in [10, 2, 1, 0, -2, 0.5, None, "b", 2L]:
        same("math.log", math.log, gen_math.log, (x, base))
same("math.log", math.log, gen_math.log, ())
same("math.log", math.log, gen_math.log, (1, 2, 3))

# module constants
for c in ["pi", "e"]:
    checks[0] += 1
    if getattr(math, c) != getattr(gen_math, c):
        fails.append("math." + c)
checks[0] += 1
if sorted(n for n in dir(math) if not n.startswith("__")) != sorted(n for n in dir(gen_math) if not n.startswith("__")):
    fails.append("math names differ")

# time
FIXED = (2020, 1, 2, 3, 4, 5, 3, 2, 0)
for fmt in ["%Y-%m-%d %H:%M:%S", "%a %b", "", "%j %w"]:
    same("time.strftime", time.strftime, gen_time.strftime, (fmt, FIXED))
    same("time.strftime", time.strftime, gen_time.strftime, (fmt, FIXED[:8]))
    same("time.strftime", time.strftime, gen_time.strftime, (fmt, None))
same("time.strftime", time.strftime, gen_time.strftime, ())
same("time.strftime", time.strftime, gen_time.strftime, (5,), kind_only=True)
same("time.sleep", time.sleep, gen_time.sleep, (0,))
same("time.sleep", time.sleep, gen_time.sleep, (0.001,))
same("time.sleep", time.sleep, gen_time.sleep, ("x",), kind_only=True)
same("time.sleep", time.sleep, gen_time.sleep, (), kind_only=True)
for name in ["time", "clock"]:
    h, g = getattr(time, name), getattr(gen_time, name)
    checks[0] += 1
    if type(h()) is not type(g()) or abs(h() - g()) > 5:
        fails.append("time." + name)
    same("time." + name, h, g, (1,), kind_only=True)
checks[0] += 1
if type(time.time) is not type(gen_time.time):
    fails.append("time.time type")

# dbg: results and the quirky error behaviour (Py_BuildException() returns None)
for name in ["Trace", "Tracen", "TraceError", "RegisterExceptionString"]:
    for args in [("msg",), (), (1,), ("a", "b"), (None,)]:
        same("dbg." + name, getattr(dbg, name), getattr(gen_dbg, name), args)
checks[0] += 1
if sorted(n for n in dir(dbg) if not n.startswith("__")) != sorted(n for n in dir(gen_dbg) if not n.startswith("__")):
    fails.append("dbg names differ")

# pack: no hand-written module in the headless host; check against the C++ semantics.
exp = [
    (gen_pack.Exist, ("a.py",), 1), (gen_pack.Exist, ("nope",), 0),
    (gen_pack.Get, ("a.py",), "print 1\n"), (gen_pack.Get, ("b.txt",), "hello\0world"),
    (gen_pack.Get, ("c.bin",), None), (gen_pack.Get, ("missing.py",), None),
    (gen_pack.Get, (), None), (gen_pack.Exist, (), None),
]
for f, args, want in exp:
    checks[0] += 1
    got = f(*args)
    if got != want:
        fails.append("pack %r%r -> %r, wanted %r" % (f, args, got, want))

print "checks: %d, failures: %d" % (checks[0], len(fails))
for f in fails[:40]:
    print "FAIL", f
if fails:
    raise SystemExit(1)
print "ALL OK"
