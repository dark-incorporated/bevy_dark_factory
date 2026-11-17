. /tmp/fleet/studyrun-common.sh
spent() { python3 - <<P
import json,glob,os
t=0.0
for f in glob.glob(os.path.expanduser("$STATE/style-*/report.json")):
    try: t+=json.load(open(f)).get("cost_total_usd") or 0
    except Exception: pass
print(f"{t:.4f}")
P
}
for k in 1 2 3 4 5; do
  if [ $((k % 2)) = 1 ]; then order="without-style with-style"; else order="with-style without-style"; fi
  for arm in $order; do
    $B/dark_factory-bench run $S --agent omp --model $MODEL --variant $arm --trials 1 --jobs 3 --run style-r$k-$arm > /tmp/fleet/studyrun-r$k-$arm.log 2>&1
    echo "round $k $arm done at $(date -u +%H:%M:%S) spend $(spent)"
    if python3 -c "import sys; sys.exit(0 if float('$(spent)')<15 else 1)"; then :; else echo "COSTSTOP"; echo rc=coststop; exit 0; fi
  done
done
echo rc=done
