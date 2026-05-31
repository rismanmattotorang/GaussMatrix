#!/bin/bash
set -eo pipefail

jsonl="tests/complement/results.jsonl"
out="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

# Some tests emit both pass and fail rows (subtests). Fold by Test, fail wins.
classify() {
	jq -sr '
		group_by(.Test)[]
		| { t: .[0].Test, s:
		      (if   any(.[]; .Action == "fail") then "error"
		       elif any(.[]; .Action == "pass") then "accept"
		       else                                  "skip"
		       end) }
		| "\(.s) \(.t)"
	' "$1"
}

snapshot_current() {
	classify "$jsonl" | sort -k2 > "$curr";
}

snapshot_baseline() {
	git show "HEAD:$jsonl" 2>/dev/null | classify /dev/stdin | sort -k2 > "$prev" || :;
}

count_status() {
	grep -c "^$1 " "$curr" || :;
}

count_lines() {
	printf '%s' "$1" | grep -c . || :;
}

delta() {
	join -j2 -t ' ' \
		<(awk -v k="$1" '$1==k' "$prev") \
		<(awk -v k="$2" '$1==k' "$curr") \
		| cut -d' ' -f1
}

grid_width() {
	awk -v n="$1" 'BEGIN { w=int(sqrt(n)); if (w*w<n) w++; print (w?w:1) }';
}

# Cells: ❎/🟩 for accept (progressed vs stable), 🟥/🟧 for error (regressed vs known),
# ⬜ skip. With no baseline (nobase=1) all fails are red and all passes plain green.
render_grid() {
	awk -v w="$1" -v nobase="$2" -v R="$3" -v P="$4" '
		BEGIN {
			split(R, x, "\n"); for (i in x) if (x[i]) reg[x[i]] = 1
			split(P, x, "\n"); for (i in x) if (x[i]) pro[x[i]] = 1
		}
		{
			t = $2
			if      ($1 == "skip")   c = "⬜"
			else if ($1 == "accept") c = pro[t]             ? "❎" : "🟩"
			else if ($1 == "error")  c = (nobase || reg[t]) ? "🟥" : "🟧"
			printf "%s", c
			if (++n % w == 0) printf "<br>"
		}
		END {
			while (n < w*w) { printf "⬛"; if (++n % w == 0) printf "<br>" }
		}
	' "$curr"
}

emit_header() {
	echo "### Complement"
	echo
	echo "| accept | errors | skipped | advanced | regressed |"
	echo "|---|---|---|---|---|"
	echo "| $accept | $error | $skip | $nprog | $nreg |"
	if test -n "$1"; then
		echo
		echo "$1"
	fi
}

emit_diff() {
	test -n "$regress$progress" || return 0
	echo
	echo '```diff'
	printf '%s\n' "$regress"  | sed -n 's/^./- &/p'
	printf '%s\n' "$progress" | sed -n 's/^./+ &/p'
	echo '```'
}

# Fetch the prior successful main.yml run's runtime-metrics artifact for this
# matrix slot. Echoes the local tar path and the run id on success, nothing on
# miss. Soft-fails: no gh, no auth, no prior run, expired artifact all yield "".
fetch_baseline_metrics() {
	command -v gh >/dev/null 2>&1 || return 0
	test -n "${feat_set:-}${sys_name:-}${sys_target:-}" || return 0

	local branch="${GITHUB_REF_NAME:-}"
	test "$branch" = "main" && return 0   # no useful comparand on main itself

	local artifact="complement_runtime_metrics-${feat_set}-${sys_name}-${sys_target}.tar.zst"
	local prev
	prev=$(gh run list \
		--workflow=main.yml \
		--branch="$branch" \
		--status=success \
		--limit=5 \
		--json databaseId \
		--jq '.[].databaseId' 2>/dev/null \
		| grep -v "^${GITHUB_RUN_ID:-}\$" \
		| head -1) || return 0
	test -n "$prev" || return 0

	local dir
	dir=$(mktemp -d)
	if ! gh run download "$prev" --name "$artifact" --dir "$dir" >/dev/null 2>&1; then
		rm -rf "$dir"
		return 0
	fi
	test -s "$dir/runtime_metrics.tar.zst" || { rm -rf "$dir"; return 0; }
	printf '%s\t%s\n' "$dir/runtime_metrics.tar.zst" "$prev"
}

emit_runtime_metrics() {
	local tar="tests/complement/runtime_metrics.tar.zst"
	test -s "$tar" || return 0
	command -v python3 >/dev/null 2>&1 || return 0

	local script="$(dirname "$0")/complement_metrics_summarise.py"
	test -x "$script" || return 0

	local pair base_tar base_run
	pair=$(fetch_baseline_metrics || true)
	base_tar=$(printf '%s' "$pair" | cut -f1)
	base_run=$(printf '%s' "$pair" | cut -f2)

	local args=(--tar "$tar" --out "$out")
	if test -n "$base_tar" && test -s "$base_tar"; then
		args+=(--baseline-tar "$base_tar" --baseline-label "Baseline (run $base_run)")
	fi
	python3 "$script" "${args[@]}" || :
	if test -n "$base_tar"; then
		rm -rf "$(dirname "$base_tar")"
	fi
}

main() {
	if test ! -s "$jsonl"; then
		echo "No results.jsonl produced." >> "$out"
		exit 0
	fi

	curr=$(mktemp); prev=$(mktemp)
	trap 'rm -f "$curr" "$prev"' EXIT
	snapshot_current
	snapshot_baseline

	accept=$(count_status accept)
	error=$( count_status error)
	skip=$(  count_status skip)
	total=$((accept + error + skip))

	regress=$( delta accept error)
	progress=$(delta error  accept)
	nreg=$( count_lines "$regress")
	nprog=$(count_lines "$progress")

	nobase=0
	test -s "$prev" || { regress= progress= nobase=1; }

	# Main-branch runs are the baseline; the grid carries no diff signal there.
	if test "${GITHUB_REF_NAME:-}" = "main"; then
		grid=
	else
		grid=$(render_grid "$(grid_width "$total")" "$nobase" "$regress" "$progress")
	fi

	{ emit_header "$grid"; emit_diff; } >> "$out"
	emit_runtime_metrics
}

main "$@"
