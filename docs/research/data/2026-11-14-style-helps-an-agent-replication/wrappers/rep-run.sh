. /tmp/fleet/studyrun2-common.sh
spent() { python3 - <<P
import json,glob,os
t=0.0
for f in glob.glob(os.path.expanduser("$STATE/rep-*/report.json")):
    try: t+=json.load(open(f)).get("cost_total_usd") or 0
    except Exception: pass
print(f"{t:.4f}")
P
}
python3 $A --selftest
cp /tmp/fleet/studyrun2-analyse-2.py docs/research/2026-11-14-style-helps-an-agent-analyse-2.py
python3 docs/research/2026-11-14-style-helps-an-agent-analyse-2.py --selftest
echo "--- step 0 rehearsal"
for k in 1 2; do for arm in without-style with-style; do
  $B/dark_factory-bench run $S --agent oracle --variant $arm --trials 1 --jobs 3 --run rep0-r$k-$arm 2>&1 | grep -E "trial\(s\)"
done; done
for arm in without-style with-style; do
  python3 $A --assemble rep0-$arm rep0-r1-$arm,rep0-r2-$arm
  $B/dark_factory-bench report $STATE/rep0-$arm --label rep0-$arm --publish /tmp/rep0/bench 2>&1 | tail -1
done
$B/dark_factory-bench compare /tmp/rep0/bench/runs/*-rep0-without-style.json /tmp/rep0/bench/runs/*-rep0-with-style.json 2>&1 | sed -n 3p
python3 $A rep0-r1-without-style,rep0-r2-without-style rep0-r1-with-style,rep0-r2-with-style | sed -n 1,6p
rm -rf $STATE/rep0-* /tmp/rep0
echo "--- step 1"
$B/dark_factory-bench run $S --agent nothing --run rep1-nothing 2>&1 | grep -E "trial\(s\)|FAIL|PASS"
for arm in with-style without-style; do $B/dark_factory-bench run $S --agent oracle --variant $arm --run rep1-oracle-$arm 2>&1 | grep -E "trial\(s\)"; done
echo "--- probe"
$B/dark_factory-bench run $S --agent omp --model $MODEL --variant with-style --task light-the-gate-at-night --trials 1 --run rep-probe 2>&1 | grep -E "trial 1|trial\(s\)"
python3 $A --routes rep-probe
echo "--- 30 runs"
for k in 1 2 3 4 5; do
  if [ $((k % 2)) = 1 ]; then order="without-style with-style"; else order="with-style without-style"; fi
  for arm in $order; do
    $B/dark_factory-bench run $S --agent omp --model $MODEL --variant $arm --trials 1 --jobs 3 --run rep-r$k-$arm > /tmp/fleet/rep-r$k-$arm.log 2>&1
    s=$(spent); echo "round $k $arm done at $(date -u +%H:%M:%S) spend $s"
    if python3 -c "import sys; sys.exit(0 if float('$s')<15 else 1)"; then :; else echo COSTSTOP; echo rc=coststop; exit 0; fi
  done
done
echo rc=done
