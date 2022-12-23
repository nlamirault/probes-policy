#!/usr/bin/env bats

@test "Accept a valid pod" {
	run kwctl run  --request-path test_data/pod_creation.json policy.wasm
	[ "$status" -eq 0 ]
	echo "$output"
	# shellcheck disable=SC2046
	[ $(expr "$output" : '.*"allowed":true.*') -ne 0 ]
}

@test "Reject invalid liveness probe" {
	run kwctl run  --request-path test_data/pod_creation_invalid_liveness.json policy.wasm
	[ "$status" -eq 0 ]
	echo "$output"
	# shellcheck disable=SC2046
	[ $(expr "$output" : '.*"allowed":false.*') -ne 0 ]
	# shellcheck disable=SC2046
	# [ $(expr "$output" : '.*"message":"pod name invalid-pod-name is not accepted".*') -ne 0 ]
}

@test "Reject invalid readiness probe" {
	run kwctl run  --request-path test_data/pod_creation_invalid_readiness.json policy.wasm
	[ "$status" -eq 0 ]
	echo "$output"
	# shellcheck disable=SC2046
	[ $(expr "$output" : '.*"allowed":false.*') -ne 0 ]
	# shellcheck disable=SC2046
	# [ $(expr "$output" : '.*"message":"pod name invalid-pod-name is not accepted".*') -ne 0 ]
}