#!/bin/bash
set -eo pipefail

BASEDIR=$(dirname "$0")

CI="${CI:-false}"
CI_VERBOSE="${CI_VERBOSE_ENV:-false}"
CI_VERBOSE_ENV="${CI_VERBOSE_ENV:-$CI_VERBOSE}"

default_cargo_profile="test"
default_feat_set="all"
default_rust_toolchain="nightly"
default_rust_target="x86_64-unknown-linux-gnu"
default_sys_name="debian"
default_sys_target="x86_64-v1-linux-gnu"
default_sys_version="testing-slim"

default_complement_verbose=0
default_complement_dirty=0
default_complement_count=1
default_complement_parallel=1
default_complement_shuffle=0
default_complement_timeout="1h"
default_complement_run=".*"

run="${1:-$default_complement_run}"

skip=""
skip="${skip}TestThreadReceiptsInSyncMSC4102"
skip="${skip}|TestToDeviceMessagesOverFederation/stopped_server"
skip="${skip}|TestRestrictedRoomsRemoteJoinFailOver"
skip="${skip}|TestRestrictedRoomsRemoteJoinFailOverInMSC3787Room"
skip="${skip}|TestToDeviceMessagesOverFederation/interrupted_connectivity"
skip="${skip}|TestUnbanViaInvite"

set -a
cargo_profile="${cargo_profile:-$default_cargo_profile}"
feat_set="${feat_set:-$default_feat_set}"
rust_target="${rust_target:-$default_rust_target}"
rust_toolchain="${rust_toolchain:-$default_rust_toolchain}"
sys_name="${sys_name:-$default_sys_name}"
sys_target="${sys_target:-$default_sys_target}"
sys_version="${sys_version:-$default_sys_version}"

runner_name=$(echo $RUNNER_NAME | cut -d"." -f1)
runner_num=$(echo $RUNNER_NAME | cut -d"." -f2)
set +a

###############################################################################

envs=""
envs="$envs -e complement_verbose=${complement_verbose:-$default_complement_verbose}"
envs="$envs -e complement_count=${complement_count:-$default_complement_count}"
envs="$envs -e complement_dirty=${complement_dirty:-$default_complement_dirty}"
envs="$envs -e complement_parallel=${complement_parallel:-$default_complement_parallel}"
envs="$envs -e complement_shuffle=${complement_shuffle:-$default_complement_shuffle}"
envs="$envs -e complement_timeout=${complement_timeout:-$default_complement_timeout}"
envs="$envs -e complement_skip=${complement_skip:-$skip}"
envs="$envs -e complement_run=${1:-$default_complement_run}"
envs="$envs -e COMPLEMENT_ALWAYS_PRINT_SERVER_LOGS=1"
envs="$envs -e COMPLEMENT_DESTROY_HS_TIMEOUT_SECS=10"

set -x
tester_image="complement-tester--${sys_name}--${sys_version}--${sys_target}"
testee_image="complement-testee--${cargo_profile}--${rust_toolchain}--${rust_target}--${feat_set}--${sys_name}--${sys_version}--${sys_target}"
name="complement_tester__${sys_name}__${sys_version}__${sys_target}"
sock="/var/run/docker.sock"
arg="--name $name -v $sock:$sock --network=host $envs $tester_image ${testee_image}"
set +x

if test "$CI_VERBOSE_ENV" = "true"; then
	date
	env
fi

docker rm -f "$name" 2>/dev/null

arg="-d $arg"
cid=$(docker run $arg)

if test "$CI" = "true"; then
	echo -n "$cid" > "$name"
fi

output_src="$cid:/usr/src/complement/full_output.jsonl"
output_dst="tests/complement/logs.jsonl"
extract_output() {
	docker cp "$output_src" "$output_dst"
}

result_src="$cid:/usr/src/complement/new_results.jsonl"
result_dst="tests/complement/results.jsonl"
extract_results() {
	docker cp "$result_src" "$result_dst"
}

metrics_archive="tests/complement/runtime_metrics.tar.zst"
extract_metrics() {
	rm -f "$metrics_archive"
	docker cp "$cid:/runtime_metrics" - 2>/dev/null \
		| zstd > "$metrics_archive" \
		|| rm -f "$metrics_archive"
}

trap 'extract_output; extract_metrics; set +x; date; echo -e "\033[1;41;37mERROR\033[0m"' ERR
trap 'docker container stop $cid; extract_output; extract_metrics' INT
docker logs -f "$cid"
docker wait "$cid" 2>/dev/null

extract_results
extract_output
extract_metrics
git diff -U0 --color --shortstat "$result_dst" | (grep "$run" || true)

git diff --quiet --exit-code "$result_dst"
echo -e "\033[1;42;30mACCEPT\033[0m"
