#!/bin/bash

# Copyright (c) Joby Aviation 2022
# Original authors: Thulio Ferraz Assis (thulio@aspect.dev), Aspect.dev
#
# Copyright (c) Thulio Ferraz Assis 2024
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -o errexit -o nounset -o pipefail

EXECROOT="${EXECROOT:-"$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"}"
export EXECROOT
export PATH="__PATH__"
args=("$@")

for i in "${!args[@]}"; do
    val="${args[i]}"

    # Make --sysroot flag absolute for GCC.
    if [[ "${val}" == "--sysroot" ]]; then
        next_index=$((i+1))
        val=${args["${next_index}"]}
        if [ -n "${EXT_BUILD_ROOT:-}" ]; then val=${val/"${EXT_BUILD_ROOT}/"/""}; fi
        if [[ "${val}" == "/"* ]]; then
            # The path already seems to be absolute.
            continue
        fi
        args["${next_index}"]="${EXECROOT}/${val}"
    fi
done

if [[ "__binary__" == *gcov && -n "${COVERAGE_DIR:-}" ]]; then
    exec > /dev/null
fi

exec "${EXECROOT}/__binary__" "${args[@]}"
